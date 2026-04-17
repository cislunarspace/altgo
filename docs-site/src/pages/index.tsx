import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';

import styles from './index.module.css';

const features = [
  {
    icon: (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/>
        <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
        <line x1="12" x2="12" y1="19" y2="22"/>
      </svg>
    ),
    title: '长按即录',
    description: '按住右 Alt 说话，松开即转写。无需切换窗口，不打断工作流。',
  },
  {
    icon: (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect width="18" height="11" x="3" y="11" rx="2" ry="2"/>
        <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
      </svg>
    ),
    title: '本地 + 云端 ASR',
    description: '支持 whisper.cpp 离线转写，完全断网可用。也支持 Whisper API 云端转写。',
  },
  {
    icon: (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/>
      </svg>
    ),
    title: 'LLM 润色',
    description: '四档润色强度，支持 OpenAI / DeepSeek / Anthropic / Ollama。',
  },
  {
    icon: (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect width="20" height="14" x="2" y="3" rx="2"/>
        <line x1="8" x2="16" y1="21" y2="21"/>
        <line x1="12" x2="12" y1="17" y2="21"/>
      </svg>
    ),
    title: '跨平台',
    description: 'Linux (X11/Wayland) · macOS · Windows 原生支持，界面体验一致。',
  },
  {
    icon: (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
        <rect width="8" height="4" x="8" y="2" rx="1" ry="1"/>
      </svg>
    ),
    title: '零侵入输出',
    description: '自动写入剪贴板，任意应用 Ctrl+V 粘贴。不依赖特定编辑器或窗口。',
  },
  {
    icon: (
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polyline points="22 7 13.5 15.5 8.5 10.5 2 17"/>
        <polyline points="16 7 22 7 22 13"/>
      </svg>
    ),
    title: '连续模式',
    description: '双击进入连续录音，适合长篇发言或会议记录。',
  },
];

export default function Home(): JSX.Element {
  const { siteConfig } = useDocusaurusContext();

  return (
    <Layout title={siteConfig.title} description={siteConfig.tagline}>
      <header className={styles.heroBanner}>
        <div className={styles.heroInner}>
          <h1 className={styles.heroTitle}>{siteConfig.title}</h1>
          <p className={styles.heroTagline}>{siteConfig.tagline}</p>
          <div className={styles.buttons}>
            <Link className="button button--primary button--lg" to="/docs/quick-start">
              快速开始
            </Link>
            <Link className="button button--outline button--lg" to="/docs/usage" style={{ borderColor: '#30363d', color: '#8b949e' }}>
              使用说明
            </Link>
          </div>
          <div className={styles.heroSteps}>
            <div className={styles.heroStep}>
              <span className={styles.heroStepIcon}>⌥</span>
              <span>按住 Alt</span>
            </div>
            <span className={styles.heroArrow}>→</span>
            <div className={styles.heroStep}>
              <span className={styles.heroStepIcon}>🎙</span>
              <span>说话</span>
            </div>
            <span className={styles.heroArrow}>→</span>
            <div className={styles.heroStep}>
              <span className={styles.heroStepIcon}>📋</span>
              <span>文字已在剪贴板</span>
            </div>
          </div>
        </div>
      </header>

      <main>
        <section className={styles.features}>
          <div className="container">
            <p className={styles.featuresHeading}>功能特点</p>
            <h2 className={styles.featuresTitle}>为你设计的细节体验</h2>
            <div className={styles.featureGrid}>
              {features.map((f, idx) => (
                <div key={idx} className={styles.feature}>
                  <div className={styles.featureCard}>
                    <div className={styles.featureIcon}>{f.icon}</div>
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
            <h2 className={styles.ctaTitle}>准备好开始了吗？</h2>
            <p className={styles.ctaSubtitle}>30 秒安装，永久提升输入效率</p>
            <Link className="button button--primary button--lg" to="/docs/quick-start">
              快速开始 →
            </Link>
          </div>
        </section>
      </main>
    </Layout>
  );
}
