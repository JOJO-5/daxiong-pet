"""Conservative prototype policy; host source and current text authorize proposals.
No database writes, no model/golden access. Unrecognized phrasing abstains.
"""
import re

STYLE_PHRASES = {
    'brief': ['简短一些', '简短一点', '简短', '短一点', '少说一点'],
    'gentle': ['温柔一些', '温柔一点', '温柔', '柔和一点'],
    'playful': ['活泼一些', '活泼一点', '活泼'],
    'quiet': ['安静一些', '安静一点', '安静', '不要太频繁', '少打扰我'],
}
NAME = r'[\w\u4e00-\u9fff·-]{1,16}'
VALUE = rf'[“「\"]?({NAME})[”」\"]?'
NAME_PATTERNS = [
    rf'(?:以后|今后|从现在起)(?:请)?(?:叫我|称呼我|喊我){VALUE}',
    rf'我希望你(?:以后|今后)(?:叫我|称呼我|喊我){VALUE}',
    rf'请记住我的称呼是{VALUE}',
    rf'(?:以后|今后)改叫我{VALUE}(?:，|,)(?:不叫|别再叫){NAME}了?',
]
STYLE_PREFIXES = [
    r'(?:以后|今后)(?:请)?说话',
    r'我希望你(?:以后|今后)说话',
]

def explicit_preference(text, source):
    """Return one authorized canonical proposal, or None. Source is host-owned."""
    if source != 'user_chat' or not isinstance(text, str):
        return None
    text = text.strip().rstrip('。！!').replace(' ', '')
    for pattern in NAME_PATTERNS:
        match = re.fullmatch(pattern, text)
        if match:
            return {'name': 'set_preferred_name', 'arguments': {'name': match.group(1)}}
    for style, phrases in STYLE_PHRASES.items():
        for prefix in STYLE_PREFIXES:
            for phrase in phrases:
                if re.fullmatch(prefix + re.escape(phrase), text):
                    return {'name': 'set_speaking_style', 'arguments': {'style': style}}
    return None

def filter_proposals(calls, text, source):
    """Validate proposals against deterministic current-message evidence.
    Never manufactures a call when the model omitted one. Reject whole batch.
    """
    if source != 'user_chat':
        return [], 'non_user_source'
    evidence = explicit_preference(text, source)
    if evidence is None:
        return [], 'no_supported_explicit_declaration'
    if not isinstance(calls, list) or len(calls) != 1:
        return [], 'missing_or_multiple_proposals'
    if calls[0] != evidence:
        return [], 'proposal_differs_from_current_evidence'
    return calls, 'authorized_current_declaration'
