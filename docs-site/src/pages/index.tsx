import React from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import {
  Mic,
  Infinity,
  Brain,
  Sparkles,
  Monitor,
  ClipboardCopy,
  ArrowRight,
  Download,
  BookOpen,
} from 'lucide-react';
import styles from './index.module.css';

const features = [
  {
    icon: Mic,
    title: '长按即录',
    description: '按住右 Alt 说话，松开即转写。无需切换窗口，不打断工作流。',
  },
  {
    icon: Infinity,
    title: '连续模式',
    description: '双击进入连续录音，适合长篇发言或会议记录。单击停止。',
  },
  {
    icon: Brain,
    title: '本地 + 云端 ASR',
    description: 'whisper.cpp 本地转写完全断网可用，也支持 Whisper API 云端转写。',
  },
  {
    icon: Sparkles,
    title: 'LLM 润色',
    description: '四档润色强度，支持 OpenAI / DeepSeek / Anthropic / Ollama。',
  },
  {
    icon: Monitor,
    title: 'Linux 原生',
    description: '支持 X11 / Wayland，通过子进程集成系统工具，构建简单。',
  },
  {
    icon: ClipboardCopy,
    title: '剪贴板 + 悬浮窗',
    description: '转写成功后写入剪贴板并弹出悬浮窗；可核对文本或再次复制。',
  },
];

const screenshots = [
  { src: '/altgo/img/screenshot-main.png', alt: '主界面' },
  { src: '/altgo/img/screenshot-home.png', alt: '首页转写中' },
  { src: '/altgo/img/screenshot-overlay.png', alt: '悬浮窗' },
  { src: '/altgo/img/screenshot-settings.png', alt: '设置页面' },
  { src: '/altgo/img/screenshot-history.png', alt: '转录历史' },
];

export default function Home(): JSX.Element {
  const { siteConfig } = useDocusaurusContext();
  const [activeShot, setActiveShot] = React.useState(0);

  React.useEffect(() => {
    const timer = setInterval(() => {
      setActiveShot((prev) => (prev + 1) % screenshots.length);
    }, 4000);
    return () => clearInterval(timer);
  }, []);

  return (
    <Layout title={siteConfig.title} description={siteConfig.tagline}>
      {/* ─── Hero ─── */}
      <header className={styles.heroBanner}>
        <div className={styles.heroGlow} />
        <div className={styles.heroGrid} />
        <div className={clsx('container', styles.heroInner)}>
          <div className={styles.heroBadge}>
            <span className={styles.heroBadgeDot} />
            跨平台语音转文字桌面工具
          </div>
          <h1 className={styles.heroTitle}>
            无需打字
            <br />
            <span className={styles.heroTitleAccent}>言出法随</span>
          </h1>
          <p className={styles.heroTagline}>
            基于 Tauri + React + Rust 的语音转文字应用。
            <br />
            按住 Alt 说话，松开后在本地用 whisper.cpp 转写，可接入 LLM 润色。
          </p>
          <div className={styles.buttons}>
            <Link
              className={clsx('button', styles.btnPrimary)}
              to="/docs/quick-start"
            >
              <Download size={18} />
              快速开始
            </Link>
            <Link
              className={clsx('button', styles.btnSecondary)}
              to="/docs/usage"
            >
              <BookOpen size={18} />
              使用说明
            </Link>
            <Link
              className={clsx('button', styles.btnGhost)}
              href="https://github.com/cislunarspace/altgo"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.403 5.403 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4"/>
                <path d="M9 18c-4.51 2-5-2-7-2"/>
              </svg>
              GitHub
            </Link>
          </div>

          {/* Screenshot showcase */}
          <div className={styles.showcase}>
            <div className={styles.showcaseFrame}>
              {screenshots.map((s, i) => (
                <img
                  key={s.src}
                  src={s.src}
                  alt={s.alt}
                  className={clsx(
                    styles.showcaseImg,
                    i === activeShot && styles.showcaseImgActive
                  )}
                />
              ))}
              <div className={styles.showcaseOverlay} />
            </div>
            <div className={styles.showcaseDots}>
              {screenshots.map((_, i) => (
                <button
                  key={i}
                  className={clsx(
                    styles.showcaseDot,
                    i === activeShot && styles.showcaseDotActive
                  )}
                  onClick={() => setActiveShot(i)}
                  aria-label={`查看截图 ${i + 1}`}
                />
              ))}
            </div>
          </div>
        </div>
      </header>

      <main>
        {/* ─── How it works ─── */}
        <section className={styles.howItWorks}>
          <div className="container">
            <p className={styles.sectionKicker}>三步上手</p>
            <h2 className={styles.sectionTitle}>简单到不需要教程</h2>
            <div className={styles.steps}>
              <div className={styles.step}>
                <div className={styles.stepNum}>1</div>
                <h3>安装并设置</h3>
                <p>
                  下载 deb / AppImage 安装包，在设置中选择转写引擎与模型。
                </p>
              </div>
              <ArrowRight className={styles.stepArrow} size={24} />
              <div className={styles.step}>
                <div className={styles.stepNum}>2</div>
                <h3>按住 Alt 说话</h3>
                <p>
                  长按右 Alt 开始录音，松开自动转写；双击进入连续模式。
                </p>
              </div>
              <ArrowRight className={styles.stepArrow} size={24} />
              <div className={styles.step}>
                <div className={styles.stepNum}>3</div>
                <h3>粘贴使用</h3>
                <p>
                  转写结果自动写入剪贴板，悬浮窗同步展示，随时可再次复制。
                </p>
              </div>
            </div>
          </div>
        </section>

        {/* ─── Features ─── */}
        <section className={styles.features}>
          <div className="container">
            <p className={styles.sectionKicker}>功能特点</p>
            <h2 className={styles.sectionTitle}>为你设计的细节体验</h2>
            <div className={styles.featureGrid}>
              {features.map((f) => {
                const Icon = f.icon;
                return (
                  <div key={f.title} className={styles.featureCard}>
                    <div className={styles.featureIcon}>
                      <Icon size={22} strokeWidth={2} />
                    </div>
                    <h3>{f.title}</h3>
                    <p>{f.description}</p>
                  </div>
                );
              })}
            </div>
          </div>
        </section>

        {/* ─── CTA ─── */}
        <section className={styles.cta}>
          <div className="container">
            <div className={styles.ctaBox}>
              <h2 className={styles.ctaTitle}>准备好提升输入效率了吗？</h2>
              <p className={styles.ctaSubtitle}>
                30 秒安装，永久改变你的输入方式
              </p>
              <div className={styles.buttons}>
                <Link
                  className={clsx('button', styles.btnPrimary)}
                  to="/docs/quick-start"
                >
                  快速开始 <ArrowRight size={18} />
                </Link>
                <Link
                  className={clsx('button', styles.btnGhost)}
                  href="https://github.com/cislunarspace/altgo/releases"
                >
                  下载最新版
                </Link>
              </div>
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}
