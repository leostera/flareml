const fmlLanguage = {
  name: 'fml',
  displayName: 'FlareML',
  scopeName: 'source.fml',
  fileTypes: ['fml'],
  patterns: [
    { include: '#comments' },
    { include: '#strings' },
    {
      match: '\\b(?:type|actor|property|check|domain|init|handle_message|let|match|forall|exists|inputs|main|fairness|once|weak|choose)\\b',
      name: 'keyword.control.fml',
    },
    {
      match: '\\b(?:always|eventually|reachable|leads_to|until|implies|and|or|not)\\b',
      name: 'keyword.operator.fml',
    },
    {
      match: '\\b(?:Int|Bool|String|Unit|unit|Option|Result|Actor)\\b',
      name: 'support.type.fml',
    },
    {
      match: '\\b(?:send|spawn|instances|messages)\\b',
      name: 'entity.name.function.fml',
    },
    {
      match: '\\b(?:spawn_bound|mailbox_bound|message_bound|runtime\\.progress)\\b',
      name: 'variable.other.constant.fml',
    },
    {
      match: '\\b[A-Z][A-Za-z0-9_]*\\b',
      name: 'entity.name.type.fml',
    },
    {
      match: '\\b[0-9]+(?:_[0-9]+)*\\b',
      name: 'constant.numeric.fml',
    },
    {
      match: '==|!=|<=|>=|->|=>|\\+|-|\\*|/|=|<|>',
      name: 'keyword.operator.fml',
    },
    {
      match: '\\b(?:true|false)\\b',
      name: 'constant.language.fml',
    },
  ],
  repository: {
    comments: {
      patterns: [
        { match: '//.*$', name: 'comment.line.double-slash.fml' },
      ],
    },
    strings: {
      name: 'string.quoted.double.fml',
      begin: '"',
      end: '"',
      patterns: [
        { match: '\\\\.', name: 'constant.character.escape.fml' },
      ],
    },
  },
};

export default fmlLanguage;
