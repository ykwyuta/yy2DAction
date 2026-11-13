# yy2DAction

## STELLA LIVE

星野あおいのライブ「きらり、ひかる。」で遊ぶブラウザ向けリズムゲームです。

```powershell
node prototype/server.mjs
```

[STELLA LIVE を開く](http://127.0.0.1:8123/live/)。
EASY または NORMAL を選び、ノーツが判定ラインに重なったら左から **D / F / J / K** を押すか、対応するボタンをタップします。**Esc** または画面のポーズボタンで一時停止できます。

8123 番ポートが使用中の場合は、PowerShell で `$env:PORT = '18123'` を設定して起動し、[18123 番のライブ画面](http://127.0.0.1:18123/live/) を開きます。

楽曲は 128 BPM、約 62 秒です。譜面、歌詞、ダンスは再生中の音声に同期します。歌声と伴奏は自作の合成音源です。

### 素材の制作

キャラクターの透過イラスト 3 枚とライブ会場の背景を、画像生成スキルの組み込み ImageGen で作成しました。ゲームでは WebP に変換した画像を使っています。

- [星野あおい・リーチ](prototype/live/assets/aoi-reach.webp) — 日本のアニメ調アイドルゲーム向けのオリジナルキャラクター。濃紺のハイポニーテール、ピンクの星形ヘアピン、白と紫の金縁ステージ衣装、ヘッドセットマイク。歌いながら右手を左上に伸ばし、足を開いて踏み出す全身ポーズ。透過背景。
- [星野あおい・ライズ](prototype/live/assets/aoi-rise.webp) — リーチ画像を参照し、顔、髪、衣装、ライティングを揃える。左足に重心を移して膝を曲げ、右手を胸に添え、左腕を右上へ伸ばす。透過背景。
- [星野あおい・ステップ](prototype/live/assets/aoi-step.webp) — リーチ画像を参照し、顔、髪、衣装、ライティングを揃える。右足に重心を移し、左のつま先を横に置く。右手は肩に、左腕は画面の右を指す。透過背景。
- [ライブ会場](prototype/live/assets/arena.webp) — 深い藍色の屋内アリーナ。シアンと紫の照明、ピンクの LED、暗い客席のペンライト、床の反射。中央に出演者を合成するための空間を設けた横長の背景。

音源を変更する場合は `prototype/live/src/song.mjs` の歌詞、音程、音価を編集します。次のコマンドで再生成できます。

```powershell
node prototype/live/render-song.mjs stella.wav
ffmpeg -y -i stella.wav -codec:a libmp3lame -q:a 3 prototype/live/assets/kirari-hikaru.mp3
```

## 既存のプロトタイプ

- [Canvas 版](http://127.0.0.1:8123/) は同じサーバーで開けます。
- Rust / WASM 版は `node engine/web/server.mjs` で起動し、[ポート 8124](http://127.0.0.1:8124/) を開きます。
