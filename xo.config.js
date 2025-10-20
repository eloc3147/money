/** @type {import('xo').FlatXoConfig} */
const xoConfig = [
    {ignores: 'old'},
    {
        space: 4,
        react: true,
        rules: {
            'react/jsx-indent': ['error', 4],
            'react/jsx-indent-props': ['error', 4],
            'react/react-in-jsx-scope': 'off',
        },
    },
];

export default xoConfig;
