# 第三方资源说明

NextMail 当前不再随安装包分发 Roboto、Droid Sans Fallback 或其他第三方字体。界面使用 Windows/macOS 自带的系统 UI 字体与中文回退字体，不复制或再分发这些系统字体文件。

若未来重新引入随包字体或其他独立资源，必须在本文件记录来源、版本和许可证，并把对应许可证文本加入发布资源。

## dnd-kit

- 版本：@dnd-kit/core 6.3.1，间接依赖 @dnd-kit/accessibility 3.1.1、@dnd-kit/utilities 3.2.2。
- 用途：文件夹长按排序、邮件拖动到文件夹、取消与边缘自动滚动。
- 来源：https://github.com/clauderic/dnd-kit
- 许可证：MIT
- Copyright (c) 2021, Claudéric Demers
- 许可证正文与下方 `react-markdown` 所列 MIT 文本相同。

## @radix-ui/react-popover

- 版本：1.1.19
- 用途：编辑器字体、字号和颜色选择浮层，提供焦点管理和键盘交互。
- 来源：https://github.com/radix-ui/primitives
- 许可证：MIT
- Copyright (c) 2022 WorkOS
- 许可证正文与下方 `react-markdown` 所列 MIT 文本相同。

## fontdb

- 版本：0.23.0
- 用途：在后台读取系统安装字体的名称；不分发字体文件。
- 来源：https://github.com/RazrFalcon/fontdb
- 许可证：MIT
- Copyright (c) 2020 Yevhenii Reizner
- 许可证正文与下方 `react-markdown` 所列 MIT 文本相同。
- 新增间接依赖 fontconfig-parser、core_maths、libm 为 MIT；ttf-parser、memmap2、roxmltree 为 MIT OR Apache-2.0；slotmap 为 Zlib，见下节。

## slotmap

- 版本：1.1.1（fontdb 的间接依赖）
- 来源：https://github.com/orlp/slotmap
- 许可证：Zlib；已于 2026-10-27 经用户确认引入。

```text
Copyright (c) 2021 Orson Peters <orsonpeters@gmail.com>

This software is provided 'as-is', without any express or implied warranty. In
no event will the authors be held liable for any damages arising from the use of
this software.

Permission is granted to anyone to use this software for any purpose, including
commercial applications, and to alter it and redistribute it freely, subject to
the following restrictions:

 1. The origin of this software must not be misrepresented; you must not claim
    that you wrote the original software. If you use this software in a product,
    an acknowledgment in the product documentation would be appreciated but is
    not required.

 2. Altered source versions must be plainly marked as such, and must not be
    misrepresented as being the original software.

 3. This notice may not be removed or altered from any source distribution.
```

## @tanstack/react-virtual

- 版本：3.14.11
- 用途：虚拟化邮件长列表，只挂载当前可视行及少量 overscan，限制同步期间的 DOM 与组件内存。
- 许可证：MIT
- Copyright (c) 2021-present Tanner Linsley
- 许可证正文与下方 `react-markdown` 所列 MIT 文本相同。

## react-markdown

- 版本：10.1.0
- 用途：在独立更新窗口中将不可信 Release notes 解析为受限 React 元素。
- 许可证：MIT
- Copyright (c) Espen Hovlandsdal

```text
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
