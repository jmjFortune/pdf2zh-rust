const staticExport = process.env.PDF2ZH_STATIC_EXPORT === "1";
const backend = (process.env.PDF2ZH_API_URL || "http://127.0.0.1:8000").replace(/\/$/, "");

module.exports = {
  ...(staticExport ? { output: "export" } : {
    async rewrites() {
      return [{ source: "/api/:path*", destination: `${backend}/api/:path*` }];
    },
  }),
};
