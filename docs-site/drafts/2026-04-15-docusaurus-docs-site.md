# Docusaurus 文档站点 + GitHub Pages 部署

## 目标
用 Docusaurus 构建 altgo 的文档网站，重新编写文档内容，通过 GitHub Actions 自动部署到 GitHub Pages。

## 任务列表

### A. 初始化 Docusaurus 项目
- [ ] 1. 在 `docs-site/` 目录下初始化 Docusaurus 项目（TypeScript 模板）
- [ ] 2. 配置 `docusaurus.config.ts`：站点名、URL（github.io）、导航栏、页脚、中文 i18n
- [ ] 3. 配置 GitHub Pages 部署设置（`url`、`baseUrl`、`trailingSlash`）

### B. 编写文档内容
- [ ] 4. 编写首页（`src/pages/index.tsx`）：Hero 区域 + 功能亮点卡片
- [ ] 5. 编写「快速开始」文档：安装步骤（Linux / Windows 分标签页）
- [ ] 6. 编写「配置指南」文档：配置文件详解、环境变量、Provider 示例
- [ ] 7. 编写「使用说明」文档：长按/双击/连续录音、润色级别、快捷键
- [ ] 8. 编写「架构」文档：流水线设计、模块说明、跨平台策略
- [ ] 9. 编写「常见问题」FAQ 文档
- [ ] 10. 配置侧边栏（`sidebars.ts`）

### C. 样式与资源
- [ ] 11. 添加 altgo 图标到 static/ 目录
- [ ] 12. 自定义主题色和 favicon

### D. CI/CD 部署
- [ ] 13. 创建 `.github/workflows/deploy-docs.yml` GitHub Actions 工作流
- [ ] 14. 本地构建验证（`npm run build`）
- [ ] 15. 推送到 GitHub 并触发部署

## 备注
- 使用 Docusaurus v3 + TypeScript
- 中文为主要语言
- 项目仓库：`cislunarspace/altgo`，GitHub Pages URL 将为 `https://cislunarspace.github.io/altgo/`
- 文档内容基于 README.md / CLAUDE.md 重新编写，不是直接搬运
- 需要安装 Node.js（>=18）
