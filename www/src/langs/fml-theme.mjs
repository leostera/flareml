const fmlTheme = {
  name: 'flareml-paper',
  type: 'light',
  colors: {
    'editor.background': '#f8f8f7',
    'editor.foreground': '#171716',
    'editorLineNumber.foreground': '#6b6b66',
    'editorLineNumber.activeForeground': '#171716',
    'editor.selectionBackground': '#e5e5e2',
  },
  tokenColors: [
    {
      scope: ['comment', 'punctuation.definition.comment'],
      settings: { foreground: '#686875', fontStyle: 'italic' },
    },
    {
      scope: ['keyword.control.fml'],
      settings: { foreground: '#7135A5', fontStyle: 'bold' },
    },
    {
      scope: ['keyword.operator.fml'],
      settings: { foreground: '#B4235A', fontStyle: 'bold' },
    },
    {
      scope: ['support.type.fml'],
      settings: { foreground: '#005B96', fontStyle: 'bold' },
    },
    {
      scope: ['entity.name.type.fml'],
      settings: { foreground: '#00695C', fontStyle: 'bold' },
    },
    {
      scope: ['entity.name.function.fml'],
      settings: { foreground: '#0F766E', fontStyle: 'bold' },
    },
    {
      scope: ['string.quoted.double.fml'],
      settings: { foreground: '#9A3412' },
    },
    {
      scope: ['constant.numeric.fml'],
      settings: { foreground: '#8A5A00' },
    },
    {
      scope: ['constant.language.fml'],
      settings: { foreground: '#08745A', fontStyle: 'bold' },
    },
    {
      scope: ['variable.other.constant.fml'],
      settings: { foreground: '#A13D08' },
    },
    {
      scope: ['constant.character.escape.fml'],
      settings: { foreground: '#7C3AED' },
    },
  ],
};

export default fmlTheme;
