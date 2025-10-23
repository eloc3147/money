/** @type {import('xo').FlatXoConfig} */
const xoConfig = [
    {ignores: 'old'},
    {
        space: 4,
        react: true,
        rules: {
            '@typescript-eslint/consistent-type-imports': ['error', {prefer: 'no-type-imports'}],
            'react/jsx-indent': ['error', 4],
            'react/jsx-indent-props': ['error', 4],
            'react/react-in-jsx-scope': 'off',
            'import-x/order': ['error', {
                'newlines-between': 'always',
                alphabetize: {order: 'asc', orderImportKind: 'asc', caseInsensitive: true},
                named: true,
                warnOnUnassignedImports: true,
            }],
        },
    },
];

export default xoConfig;
