# Formula corpus catalog

Side-by-side listing of the roundtrip-verified formulas in
`tests/roundtrip.rs`. Regenerate with
`cargo run --example catalog > docs/examples.md`.

Every formula satisfies `parse(render(normalize(x))) == normalize(x)`
and `render(parse(aa)) == aa` (the AA feeds straight into `formulaa --aa2latex`).

## MDN's three famous mathematical formulas

### cardano

```plain
  ┌─────────────── ┌──────────────
  │     ┌───────── │    ┌─────────
  │  𝑞  │ 𝑞²   𝑝³  │ 𝑞  │ 𝑞²   𝑝³
𝑡=│-───+│────+────+│───-│────+────
  ∛  2  √ 4    27  ∛ 2  √ 4    27
```

LaTeX:

```latex
t=\sqrt[3]{-\frac{q}{2}+\sqrt{\frac{q^{2}}{4}+\frac{p^{3}}{27}}}+\sqrt[3]{\frac{q}{2}-\sqrt{\frac{q^{2}}{4}+\frac{p^{3}}{27}}}
```

### cauchy-schwarz

```plain
⎛  𝑛     _ ⎞  ⎛  𝑛      ⎞⎛  𝑛      ⎞
⎜┈┈∑┈┈ 𝑢ₖ𝑣ₖ⎟²≤⎜┈┈∑┈┈ 𝑢ₖ²⎟⎜┈┈∑┈┈ 𝑣ₖ²⎟
⎝ 𝑘=1      ⎠  ⎝ 𝑘=1     ⎠⎝ 𝑘=1     ⎠
```

LaTeX:

```latex
\left(\sum_{k=1}^{n}u_{k}\bar{v}_{k}\right)^{2}\le \left(\sum_{k=1}^{n}u_{k}^{2}\right)\left(\sum_{k=1}^{n}v_{k}^{2}\right)
```

### vandermonde

```plain
⎡ 1   𝑥₁   𝑥₁²   ⋯   𝑥₁ⁿ⁻¹ ⎤
⎢   ┼    ┼     ┼   ┼       ⎥
⎢ 1   𝑥₂   𝑥₂²   ⋯   𝑥₂ⁿ⁻¹ ⎥
⎢   ┼    ┼     ┼   ┼       ⎥= ┈┈┈┈∏┈┈┈┈ (𝑥ⱼ-𝑥ᵢ)
⎢ ⋮   ⋮     ⋮    ⋱     ⋮   ⎥   1≤𝑖<𝑗≤𝑛
⎢   ┼    ┼     ┼   ┼       ⎥
⎣ 1   𝑥ₙ   𝑥ₙ²   ⋯   𝑥ₙⁿ⁻¹ ⎦
```

LaTeX:

```latex
\begin{bmatrix} 1 & x_{1} & x_{1}^{2} & \cdots  & x_{1}^{n-1} \\ 1 & x_{2} & x_{2}^{2} & \cdots  & x_{2}^{n-1} \\ \vdots  & \vdots  & \vdots  & \ddots  & \vdots  \\ 1 & x_{n} & x_{n}^{2} & \cdots  & x_{n}^{n-1} \end{bmatrix}=\prod_{1\le i<j\le n}\left(x_{j}-x_{i}\right)
```

## Physics

### gaussian

```plain
 ∞    -𝑥²   ┌─
┈∫┈┈ 𝑒   𝑑𝑥=√π
 -∞
```

LaTeX:

```latex
\int_{-\infty }^{\infty }e^{-x^{2}}dx=\sqrt{\pi }
```

### schroedinger

```plain
   ∂Ψ    ℏ²   ∂²Ψ
𝑖ℏ────=-──── ─────+𝑉(𝑥)Ψ
   ∂𝑡    2𝑚   ∂𝑥²
```

LaTeX:

```latex
i\hbar \frac{\partial \Psi }{\partial t}=-\frac{\hbar ^{2}}{2m}\frac{\partial ^{2}\Psi }{\partial x^{2}}+V\left(x\right)\Psi 
```

### gauss-law

```plain
 ￫  ￫  𝑄
∮𝐸⋅𝑑𝐴=────
       ε₀
```

LaTeX:

```latex
\oint \vec{E}\cdot d\vec{A}=\frac{Q}{\varepsilon _{0}}
```

### cauchy-integral

```plain
       1    𝑓(𝑧)
𝑓(𝑎)=─────∮──────𝑑𝑧
      2π𝑖   𝑧-𝑎
```

LaTeX:

```latex
f\left(a\right)=\frac{1}{2\pi i}\oint \frac{f\left(z\right)}{z-a}dz
```

### euler

```plain
 𝑖π
𝑒  +1=0
```

LaTeX:

```latex
e^{i\pi }+1=0
```

## Statistics

### normal-pdf

```plain
               (𝑥-μ)²
             -────────
        1       2σ²
𝑓(𝑥)=───────𝑒
      ┌────
      √2πσ²
```

LaTeX:

```latex
f\left(x\right)=\frac{1}{\sqrt{2\pi \sigma ^{2}}}e^{-\frac{\left(x-\mu \right)^{2}}{2\sigma ^{2}}}
```

### variance

```plain
    1    𝑛
σ²=─── ┈┈∑┈┈ (𝑥ᵢ-μ)²
    𝑛   𝑖=1
```

LaTeX:

```latex
\sigma ^{2}=\frac{1}{n}\sum_{i=1}^{n}\left(x_{i}-\mu \right)^{2}
```

### bayes

```plain
        𝑃(𝐵|𝐴)𝑃(𝐴)
𝑃(𝐴|𝐵)=────────────
           𝑃(𝐵)
```

LaTeX:

```latex
P\left(A|B\right)=\frac{P\left(B|A\right)P\left(A\right)}{P\left(B\right)}
```

## Structural stress tests

### rotation

```plain
  ⎡ cosθ   -sinθ ⎤
𝑅=⎢      ┼       ⎥
  ⎣ sinθ   cosθ  ⎦
```

LaTeX:

```latex
R=\begin{bmatrix} \operatorname{cos}\theta  & -\operatorname{sin}\theta  \\ \operatorname{sin}\theta  & \operatorname{cos}\theta  \end{bmatrix}
```

### matrix-exponential

```plain
 ⎡ 0    1 ⎤
 ⎢    ┼   ⎥𝑡
 ⎣ -1   0 ⎦
𝑒
```

LaTeX:

```latex
e^{\begin{bmatrix} 0 & 1 \\ -1 & 0 \end{bmatrix}t}
```

### nested-matrices

```plain
⎡     1          ⎤
⎢    ───      0  ⎥
⎢     2          ⎥
⎢           ┼    ⎥
⎢ ⎡ 𝑎   𝑏 ⎤      ⎥
⎢ ⎢   ┼   ⎥   𝑥² ⎥
⎣ ⎣ 𝑐   𝑑 ⎦      ⎦
```

LaTeX:

```latex
\begin{bmatrix} \frac{1}{2} & 0 \\ \begin{bmatrix} a & b \\ c & d \end{bmatrix} & x^{2} \end{bmatrix}
```

### continued-fraction

```plain
         1
1+───────────────
          1
   1+───────────
           1
      1+───────
            1
         1+───
            𝑥
```

LaTeX:

```latex
1+\frac{1}{1+\frac{1}{1+\frac{1}{1+\frac{1}{x}}}}
```

### nested-limits

```plain
     𝑛
    ───
     2
┈┈┈┈┈∑┈┈┈┈┈ 𝑎ᵢ
 𝑖∈ ┈⋃┈ 𝑆ₖ
     𝑘
```

LaTeX:

```latex
\sum_{i\in \bigcup_{k}S_{k}}^{\frac{n}{2}}a_{i}
```

## Delimiters

### cases-abs

```plain
    ⎧┌    ┬     ┐┊
    ⎪  𝑥    𝑥≥0  ┊
⎢𝑥⎥=⎨├    ┼     ┤┊
    ⎪  -𝑥   𝑥<0  ┊
    ⎩└    ┴     ┘┊
```

LaTeX:

```latex
\left|x\right|=\begin{cases} x & x\ge 0 \\ -x & x<0 \end{cases}
```

### braket

```plain
⟨ψ│𝐻│ψ⟩
```

LaTeX:

```latex
\left\langle \psi \middle|H\middle|\psi \right\rangle 
```

### set-builder

```plain
⎧ │    1 ⎫
⎨𝑥│𝑥²>───⎬
⎩ │    2 ⎭
```

LaTeX:

```latex
\left\{x\middle|x^{2}>\frac{1}{2}\right\}
```

### interval

```plain
(0,1]
```

LaTeX:

```latex
\left(0,1\right]
```

### bare-array

```plain
┌   ┬   ┐
  𝑎   𝑏
├   ┼   ┤
  𝑐   𝑑
└   ┴   ┘
```

LaTeX:

```latex
\begin{matrix} a & b \\ c & d \end{matrix}
```

## Arrows and text

### xrightarrow

```plain
   𝑓
𝐴─────>𝐵
  𝑛→∞
```

LaTeX:

```latex
A\xrightarrow[n\to \infty ]{f}B
```

### mathrm-dx

```plain
∫𝑓(𝑥)dx
```

LaTeX:

```latex
\int f\left(x\right)\operatorname{dx}
```

## Limit-taking functions

### lim

```plain
┈lim┈ 𝑓(𝑥)
 𝑥→0
```

LaTeX:

```latex
\operatorname*{lim}_{x\to 0}f\left(x\right)
```

### argmax

```plain
┈argmax┈ 𝑓(𝑥)
  𝑥∈𝑆
```

LaTeX:

```latex
\operatorname*{arg\,max}_{x\in S}f\left(x\right)
```

## Braces

### overbrace

```plain
  𝑛
╭───╮
 𝑎+𝑏 + 𝑐
      ╰─╯
       𝑚
```

LaTeX:

```latex
\overbrace{a+b}^{n}+\underbrace{c}_{m}
```

<!-- regenerated by examples/catalog.rs -->
