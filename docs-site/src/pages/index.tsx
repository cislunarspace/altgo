import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';

import styles from './index.module.css';

function HeroBanner() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <h1 className="hero__title">{siteConfig.title}</h1>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <div className={styles.buttons}>
          <Link className="button button--secondary button--lg" to="/docs/quick-start">
            快速开始
          </Link>
          <Link className="button button--outline button--primary button--lg" to="/docs/usage">
            使用说明
          </Link>
        </div>
        <div className={styles.demo}>
          <code>按住右 Alt → 说话 → 松开 → 文字已在剪贴板</code>
        </div>
      </div>
    </header>
  );
}

const features = [
  {
    title: '🎙️ 长按即录',
    description: '按住右 Alt 说话，松开即转写。无需切换窗口。',
  },
  {
    title: '🧠 本地 + 云端 ASR',
    description: '支持 whisper.cpp 离线转写和 Whisper API 云端转写。',
  },
  {
    title: '✨ LLM 润色',
    description: '四档润色强度，支持 OpenAI / DeepSeek / Anthropic / Ollama。',
  },
  {
    title: '💻 跨平台',
    description: 'Linux (X11/Wayland) · macOS · Windows 原生支持。',
  },
  {
    title: '📋 零侵入',
    description: '自动写入剪贴板，任意应用 Ctrl+V 粘贴。',
  },
  {
    title: '♾️ 连续模式',
    description: '双击进入连续录音，适合长篇发言。',
  },
];

export default function Home() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout title={siteConfig.title} description={siteConfig.tagline}>
      <HeroBanner />
      <main>
        <section className={styles.features}>
          <div className="container">
            <div className="row">
              {features.map((f, idx) => (
                <div key={idx} className={clsx('col col--4', styles.feature)}>
                  <div className={styles.featureCard}>
                    <h3>{f.title}</h3>
                    <p>{f.description}</p>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </section>
        <section className={styles.cta}>
          <div className="container text--center">
            <h2>准备好开始了吗？</h2>
            <Link className="button button--primary button--lg" to="/docs/quick-start">
              30 秒安装
            </Link>
          </div>
        </section>
      </main>
    </Layout>
  );
}
