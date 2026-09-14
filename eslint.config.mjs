import js from '@eslint/js';
import globals from 'globals';

export default [
  {
    ignores: [
      'assets/hljs/**',
      'assets/katex/**',
      'assets/mermaid/**',
      'mobile/shared/vendor/**',
      // 构建产物：Android 打包会把手机渲染层拷进 build/ 下
      'mobile/android/**/build/**',
      'mobile/android/.gradle/**',
      'mobile/ios/build/**',
      'node_modules/**',
      'target/**',
      'dist/**',
    ],
  },
  js.configs.recommended,
  {
    rules: {
      // catch 参数和占位参数经常故意不用；空 catch 是明确的忽略分支
      'no-unused-vars': [
        'error',
        { caughtErrors: 'none', argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
      'no-empty': ['error', { allowEmptyCatch: true }],
    },
  },
  {
    // 桌面预览页脚本和手机端渲染层都是浏览器里的经典脚本
    files: ['frontend/**/*.js', 'mobile/shared/*.js'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'script',
      // hljs 由 Rust 在首帧后动态注入
      globals: { ...globals.browser, hljs: 'readonly' },
    },
  },
  {
    // 验证脚本跑在 Node 里，page.evaluate 回调则在浏览器里执行，两套全局都要放开
    files: ['scripts/*.mjs', 'mobile/scripts/*.mjs'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.node, ...globals.browser },
    },
  },
];
