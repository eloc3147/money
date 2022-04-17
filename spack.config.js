const { config } = require("@swc/core/spack");

module.exports = config({
    entry: {
        web: __dirname + "/web/money.ts",
    },
    output: {
        path: __dirname + "/static",
        module: {},
    },
});
