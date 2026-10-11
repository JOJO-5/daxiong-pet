// CPU-only OCR worker. PNG arrives on stdin; bounded JSON leaves on stdout.
// Detection/recognition preprocessing follows RapidOCR 3.10.0 (Apache-2.0).
#include <onnxruntime_cxx_api.h>
#include <opencv2/core.hpp>
#include <opencv2/imgproc.hpp>
#include <opencv2/imgcodecs.hpp>
#include "clipper.hpp"
#include "json.hpp"
#include <algorithm>
#include <array>
#include <cmath>
#include <iostream>
#include <filesystem>
#include <sstream>
#include <vector>
using json = nlohmann::json;
using Box = std::array<cv::Point2f, 4>;
static Box ordered(const cv::RotatedRect &r) {
    Box p; r.points(p.data());
    std::sort(p.begin(), p.end(), [](auto a,auto b){return a.x<b.x;});
    if(p[0].y>p[1].y)std::swap(p[0],p[1]);
    if(p[2].y>p[3].y)std::swap(p[2],p[3]);
    return {p[0],p[2],p[3],p[1]};
}
static std::vector<float> tensor(const cv::Mat &img,int height,int width,int actual_width) {
    cv::Mat resized;cv::resize(img,resized,{actual_width,height});
    std::vector<float> data(3*height*width,0);
    for(int y=0;y<height;y++)for(int x=0;x<actual_width;x++)for(int c=0;c<3;c++)
        data[c*height*width+y*width+x]=(resized.at<cv::Vec3b>(y,x)[c]/255.f-.5f)/.5f;
    return data;
}
static Ort::Value run(Ort::Session &session,std::vector<float> &data,int h,int w) {
    auto memory=Ort::MemoryInfo::CreateCpu(OrtArenaAllocator,OrtMemTypeDefault);
    std::array<int64_t,4> shape{1,3,h,w};
    auto input=Ort::Value::CreateTensor<float>(memory,data.data(),data.size(),shape.data(),4);
    Ort::AllocatorWithDefaultOptions allocator;
    auto in=session.GetInputNameAllocated(0,allocator),out=session.GetOutputNameAllocated(0,allocator);
    const char *ins[]={in.get()},*outs[]={out.get()};
    return std::move(session.Run(Ort::RunOptions{nullptr},ins,&input,1,outs,1)[0]);
}
static std::vector<Box> detect(Ort::Session &session,const cv::Mat &image) {
    double ratio=std::max(1.,736./std::min(image.rows,image.cols));
    int h=std::max(32,(int)std::nearbyint(int(image.rows*ratio)/32.)*32);
    int w=std::max(32,(int)std::nearbyint(int(image.cols*ratio)/32.)*32);
    auto data=tensor(image,h,w,w);auto output=run(session,data,h,w);
    auto shape=output.GetTensorTypeAndShapeInfo().GetShape();
    if(shape.size()!=4 || shape[0]!=1 || shape[1]!=1)throw std::runtime_error("detector_shape");
    int oh=(int)shape[2],ow=(int)shape[3];
    cv::Mat prob(oh,ow,CV_32F,output.GetTensorMutableData<float>()),mask;
    cv::compare(prob,.3,mask,cv::CMP_GT);cv::dilate(mask,mask,cv::Mat::ones(2,2,CV_8U));
    std::vector<std::vector<cv::Point>> contours;cv::findContours(mask,contours,cv::RETR_LIST,cv::CHAIN_APPROX_SIMPLE);
    std::vector<Box> boxes;
    for(size_t i=0;i<std::min<size_t>(contours.size(),1000);i++) {
        auto rect=cv::minAreaRect(contours[i]);if(std::min(rect.size.width,rect.size.height)<3)continue;
        auto box=ordered(rect);std::vector<cv::Point> polygon;
        for(auto p:box)polygon.push_back({(int)p.x,(int)p.y});
        cv::Mat region=cv::Mat::zeros(oh,ow,CV_8U);cv::fillPoly(region,std::vector<std::vector<cv::Point>>{polygon},1);
        if(cv::mean(prob,region)[0]<.5)continue;
        ClipperLib::Path path;for(auto p:box)path.emplace_back((int64_t)p.x,(int64_t)p.y);
        double perimeter=0;for(int k=0;k<4;k++)perimeter+=cv::norm(box[k]-box[(k+1)%4]);
        if(perimeter<=0)continue;
        ClipperLib::ClipperOffset offset;offset.AddPath(path,ClipperLib::jtRound,ClipperLib::etClosedPolygon);
        ClipperLib::Paths paths;offset.Execute(paths,std::abs(cv::contourArea(std::vector<cv::Point2f>(box.begin(),box.end())))*1.6/perimeter);
        std::vector<cv::Point2f> expanded;for(auto &part:paths)for(auto p:part)expanded.push_back({(float)p.X,(float)p.Y});
        if(expanded.size()<4)continue;
        rect=cv::minAreaRect(expanded);if(std::min(rect.size.width,rect.size.height)<5)continue;
        box=ordered(rect);
        for(auto &p:box){p.x=std::clamp((float)std::nearbyint(p.x/ow*image.cols),0.f,(float)image.cols-1);p.y=std::clamp((float)std::nearbyint(p.y/oh*image.rows),0.f,(float)image.rows-1);}
        if(cv::norm(box[0]-box[1])<=3||cv::norm(box[0]-box[3])<=3)continue;
        boxes.push_back(box);
    }
    std::stable_sort(boxes.begin(),boxes.end(),[](auto a,auto b){return a[0].y<b[0].y;});
    size_t start=0;while(start<boxes.size()){size_t end=start+1;while(end<boxes.size()&&boxes[end][0].y-boxes[end-1][0].y<10)end++;std::stable_sort(boxes.begin()+start,boxes.begin()+end,[](auto a,auto b){return a[0].x<b[0].x;});start=end;}
    return boxes;
}
static cv::Mat crop(const cv::Mat &image,const Box &box) {
    int w=std::max(1,(int)std::max(cv::norm(box[0]-box[1]),cv::norm(box[2]-box[3])));
    int h=std::max(1,(int)std::max(cv::norm(box[0]-box[3]),cv::norm(box[1]-box[2])));
    Box target={cv::Point2f(0,0),cv::Point2f(w,0),cv::Point2f(w,h),cv::Point2f(0,h)};
    auto transform=cv::getPerspectiveTransform(box.data(),target.data());cv::Mat result;
    cv::warpPerspective(image,result,transform,{w,h},cv::INTER_CUBIC,cv::BORDER_REPLICATE);
    if((float)h/w>=1.5)cv::rotate(result,result,cv::ROTATE_90_COUNTERCLOCKWISE);
    return result;
}
#ifdef _WIN32
int wmain(int argc,wchar_t **argv) {
#else
int main(int argc,char **argv) {
#endif
    try {
        if(argc!=4)throw std::runtime_error("expected_detector_recognizer_threads");
        int threads=std::stoi(argv[3]);if(threads<1||threads>4)throw std::runtime_error("invalid_threads");
        cv::setNumThreads(1);Ort::Env env(ORT_LOGGING_LEVEL_ERROR,"daxiong-ocr");Ort::SessionOptions options;
        options.SetIntraOpNumThreads(threads);options.SetInterOpNumThreads(1);options.DisableCpuMemArena();
        options.SetGraphOptimizationLevel(GraphOptimizationLevel::ORT_ENABLE_ALL);
        // No GPU execution provider is appended.
        Ort::Session det(env,std::filesystem::path(argv[1]).c_str(),options),rec(env,std::filesystem::path(argv[2]).c_str(),options);
        Ort::AllocatorWithDefaultOptions allocator;auto dictionary=rec.GetModelMetadata().LookupCustomMetadataMapAllocated("character",allocator);
        if(!dictionary)throw std::runtime_error("missing_embedded_dictionary");
        std::vector<std::string> chars{""};std::istringstream stream(dictionary.get());std::string line;
        while(std::getline(stream,line)){if(!line.empty()&&line.back()=='\r')line.pop_back();chars.push_back(line);}chars.push_back(" ");
        std::vector<uchar> bytes;char chunk[65536];while(std::cin.read(chunk,sizeof(chunk))||std::cin.gcount()){bytes.insert(bytes.end(),chunk,chunk+std::cin.gcount());if(bytes.size()>16*1024*1024)throw std::runtime_error("image_too_large");}
        auto image=cv::imdecode(bytes,cv::IMREAD_COLOR);if(image.empty()||image.total()>8*1024*1024)throw std::runtime_error("invalid_image");
        json lines=json::array();for(auto &box:detect(det,image)) {
            auto part=crop(image,box);int actual=std::max(1,(int)std::ceil(48.*part.cols/part.rows));int width=std::max(320,actual);
            if(width>8192)continue;
            auto data=tensor(part,48,width,actual);auto output=run(rec,data,48,width);auto shape=output.GetTensorTypeAndShapeInfo().GetShape();
            if(shape.size()!=3||shape[0]!=1||shape[2]!=(int64_t)chars.size())throw std::runtime_error("dictionary_shape_mismatch");
            auto p=output.GetTensorData<float>();std::string text;double score=0;int count=0,last=-1;
            for(int64_t t=0;t<shape[1];t++){auto row=p+t*shape[2];int id=(int)(std::max_element(row,row+shape[2])-row);if(id!=0&&id!=last){text+=chars[id];score+=row[id];count++;}last=id;}
            if(!count||score/count<.5)continue;
            json coords=json::array();for(auto p:box)coords.push_back({p.x,p.y});
            lines.push_back({{"text",text},{"confidence",score/count},{"box",coords}});
            if(lines.size()>=128)break;
        }
        std::cout<<json({{"version",1},{"provider","cpu"},{"lines",lines}}).dump()<<std::endl;
        return 0;
    }catch(const std::exception &e){std::cerr<<"OCR worker failed"<<std::endl;return 2;}
}
