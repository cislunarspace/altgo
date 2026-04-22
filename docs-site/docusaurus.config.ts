import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'altgo',
  tagline: '无需打字，言出法随',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: 'https://cislunarspace.github.io',
  baseUrl: '/altgo/',
  trailingSlash: false,

  organizationName: 'cislunarspace',
  projectName: 'altgo',

  onBrokenLinks: 'throw',

  i18n: {
    defaultLocale: 'zh-Hans',
    locales: ['zh-Hans'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/cislunarspace/altgo/tree/main/docs-site/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    metadata: [
      {
        name: 'description',
        content:
          'altgo：跨平台语音转文字桌面工具（Tauri）。whisper.cpp 本地转写，可选 OpenAI 兼容 LLM 润色；剪贴板与悬浮窗输出。以 Linux 为第一目标平台。',
      },
    ],
    image: 'img/docusaurus-social-card.jpg',
    colorMode: {
      defaultMode: 'dark',
      respectPrefersColorScheme: true,
    },
    navbar: {
      hideOnScroll: false,
      title: 'altgo',
      logo: {
        alt: 'altgo',
        src: 'img/logo.svg',
      },
      style: 'dark',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: '文档',
        },
        {
          href: 'https://github.com/cislunarspace/altgo',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: '文档',
          items: [
            {label: '快速开始', to: '/docs/quick-start'},
            {label: '配置指南', to: '/docs/configuration'},
            {label: '使用说明', to: '/docs/usage'},
          ],
        },
        {
          title: '更多',
          items: [
            {label: '架构设计', to: '/docs/architecture'},
            {label: '常见问题', to: '/docs/faq'},
          ],
        },
        {
          title: '社区',
          items: [
            {label: 'GitHub', href: 'https://github.com/cislunarspace/altgo'},
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} cislunarspace. MIT License. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['toml', 'bash', 'powershell'],
    },
    docs: {
      sidebar: {
        hideable: true,
      },
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
