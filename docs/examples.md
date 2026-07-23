# 数式コーパス カタログ

`tests/roundtrip.rs` のラウンドトリップ検証済み数式の対照表。
`cargo run --example catalog > docs/examples.md` で再生成する。

すべての式で `parse(render(normalize(x))) == normalize(x)` と
`render(parse(aa)) == aa` が成立している(AA はそのまま `mascii aa2tex` に入力可能)。

## MDN「三つの有名な数式」

### cardano

```
  ┌─────────────── ┌──────────────
  │     ┌───────── │    ┌─────────
  │  𝑞  │ 𝑞²   𝑝³  │ 𝑞  │ 𝑞²   𝑝³
𝑡=│−───+│────+────+│───−│────+────
  ∛  2  √ 4    27  ∛ 2  √ 4    27
```

LaTeX:

```latex
t=\sqrt[3]{-\frac{q}{2}+\sqrt{\frac{q^{2}}{4}+\frac{p^{3}}{27}}}+\sqrt[3]{\frac{q}{2}-\sqrt{\frac{q^{2}}{4}+\frac{p^{3}}{27}}}
```

Typst:

```typst
t = root(3, - q/2 + sqrt((q ^2)/4 + (p ^3)/(27))) + root(3, q/2 - sqrt((q ^2)/4 + (p ^3)/(27)))
```

### cauchy-schwarz

```
⎛  𝑛     _ ⎞  ⎛  𝑛      ⎞⎛  𝑛      ⎞
⎜┈┈∑┈┈ 𝑢ₖ𝑣ₖ⎟²≤⎜┈┈∑┈┈ 𝑢ₖ²⎟⎜┈┈∑┈┈ 𝑣ₖ²⎟
⎝ 𝑘=1      ⎠  ⎝ 𝑘=1     ⎠⎝ 𝑘=1     ⎠
```

LaTeX:

```latex
\left(\sum _{k=1}^{n}u_{k}\bar{v}_{k}\right)^{2}\le \left(\sum _{k=1}^{n}u_{k}^{2}\right)\left(\sum _{k=1}^{n}v_{k}^{2}\right)
```

Typst:

```typst
lr(( ∑_(k = 1)^n u _k macron(v) _k )) ^2 ≤ lr(( ∑_(k = 1)^n u _k ^2 )) lr(( ∑_(k = 1)^n v _k ^2 ))
```

### vandermonde

```
⎡ 1   𝑥₁   𝑥₁²   ⋯   𝑥₁ⁿ⁻¹ ⎤
⎢   ┼    ┼     ┼   ┼       ⎥
⎢ 1   𝑥₂   𝑥₂²   ⋯   𝑥₂ⁿ⁻¹ ⎥
⎢   ┼    ┼     ┼   ┼       ⎥= ┈┈┈┈∏┈┈┈┈ (𝑥ⱼ−𝑥ᵢ)
⎢ ⋮   ⋮     ⋮    ⋱     ⋮   ⎥   1≤𝑖<𝑗≤𝑛
⎢   ┼    ┼     ┼   ┼       ⎥
⎣ 1   𝑥ₙ   𝑥ₙ²   ⋯   𝑥ₙⁿ⁻¹ ⎦
```

LaTeX:

```latex
\begin{bmatrix} 1 & x_{1} & x_{1}^{2} & \cdots  & x_{1}^{n-1} \\ 1 & x_{2} & x_{2}^{2} & \cdots  & x_{2}^{n-1} \\ \vdots  & \vdots  & \vdots  & \ddots  & \vdots  \\ 1 & x_{n} & x_{n}^{2} & \cdots  & x_{n}^{n-1} \end{bmatrix}=\prod _{1\le i<j\le n}\left(x_{j}-x_{i}\right)
```

Typst:

```typst
mat(delim: "[", 1, x _1, x _1 ^2, ⋯, x _1 ^(n - 1); 1, x _2, x _2 ^2, ⋯, x _2 ^(n - 1); ⋮, ⋮, ⋮, ⋱, ⋮; 1, x _n, x _n ^2, ⋯, x _n ^(n - 1)) = ∏_(1 ≤ i < j ≤ n) lr(( x _j - x _i ))
```

## 物理

### gaussian

```
 ∞    −𝑥²   ┌─
┈∫┈┈ 𝑒   𝑑𝑥=√π
 −∞
```

LaTeX:

```latex
\int _{-\infty }^{\infty }e^{-x^{2}}dx=\sqrt{\pi }
```

Typst:

```typst
∫_(- ∞)^∞ e ^(- x ^2) d x = sqrt(π)
```

### schroedinger

```
   ∂Ψ    ℏ²   ∂²Ψ
𝑖ℏ────=−──── ─────+𝑉(𝑥)Ψ
   ∂𝑡    2𝑚   ∂𝑥²
```

LaTeX:

```latex
i\hbar \frac{\partial \Psi }{\partial t}=-\frac{\hbar ^{2}}{2m}\frac{\partial ^{2}\Psi }{\partial x^{2}}+V\left(x\right)\Psi 
```

Typst:

```typst
i ℏ (∂ Ψ)/(∂ t) = - (ℏ ^2)/(2 m) (∂ ^2 Ψ)/(∂ x ^2) + V lr(( x )) Ψ
```

### gauss-law

```
 ￫  ￫  𝑄
∮𝐸⋅𝑑𝐴=────
       ε₀
```

LaTeX:

```latex
\oint \vec{E}\cdot d\vec{A}=\frac{Q}{\epsilon _{0}}
```

Typst:

```typst
∮ arrow(E) ⋅ d arrow(A) = Q/(ε _0)
```

### cauchy-integral

```
       1    𝑓(𝑧)
𝑓(𝑎)=─────∮──────𝑑𝑧
      2π𝑖   𝑧−𝑎
```

LaTeX:

```latex
f\left(a\right)=\frac{1}{2\pi i}\oint \frac{f\left(z\right)}{z-a}dz
```

Typst:

```typst
f lr(( a )) = 1/(2 π i) ∮ (f lr(( z )))/(z - a) d z
```

### euler

```
 𝑖π
𝑒  +1=0
```

LaTeX:

```latex
e^{i\pi }+1=0
```

Typst:

```typst
e ^(i π) + 1 = 0
```

## 統計

### normal-pdf

```
               (𝑥−μ)²
             −────────
        1       2σ²
𝑓(𝑥)=───────𝑒
      ┌────
      √2πσ²
```

LaTeX:

```latex
f\left(x\right)=\frac{1}{\sqrt{2\pi \sigma ^{2}}}e^{-\frac{\left(x-\mu \right)^{2}}{2\sigma ^{2}}}
```

Typst:

```typst
f lr(( x )) = 1/(sqrt(2 π σ ^2)) e ^(- (lr(( x - μ )) ^2)/(2 σ ^2))
```

### variance

```
    1    𝑛
σ²=─── ┈┈∑┈┈ (𝑥ᵢ−μ)²
    𝑛   𝑖=1
```

LaTeX:

```latex
\sigma ^{2}=\frac{1}{n}\sum _{i=1}^{n}\left(x_{i}-\mu \right)^{2}
```

Typst:

```typst
σ ^2 = 1/n ∑_(i = 1)^n lr(( x _i - μ )) ^2
```

### bayes

```
        𝑃(𝐵|𝐴)𝑃(𝐴)
𝑃(𝐴|𝐵)=────────────
           𝑃(𝐵)
```

LaTeX:

```latex
P\left(A|B\right)=\frac{P\left(B|A\right)P\left(A\right)}{P\left(B\right)}
```

Typst:

```typst
P lr(( A | B )) = (P lr(( B | A )) P lr(( A )))/(P lr(( B )))
```

## 構造ストレステスト

### rotation

```
  ⎡ cosθ   −sinθ ⎤
𝑅=⎢      ┼       ⎥
  ⎣ sinθ   cosθ  ⎦
```

LaTeX:

```latex
R=\begin{bmatrix} \cos \theta  & -\sin \theta  \\ \sin \theta  & \cos \theta  \end{bmatrix}
```

Typst:

```typst
R = mat(delim: "[", cos θ, - sin θ; sin θ, cos θ)
```

### matrix-exponential

```
 ⎡ 0    1 ⎤
 ⎢    ┼   ⎥𝑡
 ⎣ −1   0 ⎦
𝑒
```

LaTeX:

```latex
e^{\begin{bmatrix} 0 & 1 \\ -1 & 0 \end{bmatrix}t}
```

Typst:

```typst
e ^(mat(delim: "[", 0, 1; - 1, 0) t)
```

### nested-matrices

```
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

Typst:

```typst
mat(delim: "[", 1/2, 0; mat(delim: "[", a, b; c, d), x ^2)
```

### cancel-simple

```
 𝑥𝑦̸
────=𝑥
 𝑦̸
```

LaTeX:

```latex
\frac{x\cancel{y}}{\cancel{y}}=x
```

Typst:

```typst
(x cancel(y))/(cancel(y)) = x
```

### cancel-frac

```
 𝑎̸+̸𝑏̸
─̸─̸─̸─̸─̸𝑑̸+𝑒
  𝑐̸
```

LaTeX:

```latex
\cancel{\frac{a+b}{c}d}+e
```

Typst:

```typst
cancel((a + b)/c d) + e
```

### cancel-in-sup

```
 𝑥2̸α̸
𝑒
```

LaTeX:

```latex
e^{x\cancel{2\alpha }}
```

Typst:

```typst
e ^(x cancel(2 α))
```

### continued-fraction

```
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

Typst:

```typst
1 + 1/(1 + 1/(1 + 1/(1 + 1/x)))
```

### nested-limits

```
     𝑛
    ───
     2
┈┈┈┈┈∑┈┈┈┈┈ 𝑎ᵢ
 𝑖∈ ┈⋃┈ 𝑆ₖ
     𝑘
```

LaTeX:

```latex
\sum _{i\in \bigcup _{k}S_{k}}^{\frac{n}{2}}a_{i}
```

Typst:

```typst
∑_(i ∈ ⋃_k S _k)^(n/2) a _i
```

## デリミタ

### cases-abs

```
    ⎧┌    ┬     ┐┊
    ⎪  𝑥    𝑥≥0  ┊
⎢𝑥⎥=⎨├    ┼     ┤┊
    ⎪  −𝑥   𝑥<0  ┊
    ⎩└    ┴     ┘┊
```

LaTeX:

```latex
\left|x\right|=\begin{cases} x & x\ge 0 \\ -x & x<0 \end{cases}
```

Typst:

```typst
lr(| x |) = cases(x & x ≥ 0, - x & x < 0)
```

### braket

```
⟨ψ│𝐻│ψ⟩
```

LaTeX:

```latex
\left\langle \psi \middle|H\middle|\psi \right\rangle 
```

Typst:

```typst
lr(angle.l ψ mid(|) H mid(|) ψ angle.r)
```

### set-builder

```
⎧ │    1 ⎫
⎨𝑥│𝑥²>───⎬
⎩ │    2 ⎭
```

LaTeX:

```latex
\left\{x\middle|x^{2}>\frac{1}{2}\right\}
```

Typst:

```typst
lr({ x mid(|) x ^2 > 1/2 })
```

### interval

```
(0,1]
```

LaTeX:

```latex
\left(0,1\right]
```

Typst:

```typst
lr(( 0 , 1 ])
```

### bare-array

```
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

Typst:

```typst
mat(delim: #none, a, b; c, d)
```

## 矢印・テキスト

### xrightarrow

```
   𝑓
𝐴─────>𝐵
  𝑛→∞
```

LaTeX:

```latex
A\xrightarrow[n\to \infty ]{f}B
```

Typst:

```typst
A attach(stretch(->), t: (f), b: (n → ∞)) B
```

### mathrm-dx

```
∫𝑓(𝑥)dx
```

LaTeX:

```latex
\int f\left(x\right)\mathrm{dx}
```

Typst:

```typst
∫ f lr(( x )) upright("dx")
```

## 極限関数

### lim

```
┈lim┈ 𝑓(𝑥)
 𝑥→0
```

LaTeX:

```latex
\lim _{x\to 0}f\left(x\right)
```

Typst:

```typst
lim_(x → 0) f lr(( x ))
```

### argmax

```
┈arg┈max┈ 𝑓(𝑥)
   𝑥∈𝑆
```

LaTeX:

```latex
\mathop{\arg \max}_{x\in S}f\left(x\right)
```

Typst:

```typst
op("arg max", limits: #true)_(x ∈ S) f lr(( x ))
```

## ブレース

### overbrace

```
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

Typst:

```typst
overbrace(a + b, n) + underbrace(c, m)
```

