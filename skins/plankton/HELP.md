# Plankton

> O plano infalível pra roubar a fórmula do código.

Um tema do AIC com a cara do Plankton, do Bob Esponja. Tudo funciona igual ao tema AIC (blocos, tmux, board, captura rápida, worktrees, tokens); o manual completo está na aba **AIC**. Aqui está só o que muda.

## Estados

| Estado | Nome no Plankton | Quando |
| --- | --- | --- |
| BOOT | LIGANDO | a sessão está abrindo |
| WORKING | TRAMANDO | o Claude está trabalhando |
| IDLE | À ESPREITA | esperando você digitar |
| WAITING | KAREN?! | pediu permissão ou fez uma pergunta |
| DONE | MUAHAHA! | terminou e você ainda não viu |
| GAME OVER | DERROTADO | o processo morreu |

## Visual

- **Mascote**: o Plankton desenhado em vetor, animado conforme o estado.
- **Fundo**: bolhas da Fenda do Biquíni subindo.
- **Letreiro**: letras verdes com contorno grosso, com o "O" virando o olho do Plankton.
- **Cantos arredondados** e sombra suave.

Variantes (troque com **Ctrl+Shift+Y**):

- **balde**: o Balde de Lixo. Metal escuro, verde Plankton e o vermelho do olho (padrão).
- **karen**: o computador-esposa. Fósforo verde-água num CRT.

## Som

O som padrão de "terminou" é o gemido do Plankton (*"Aaaahhhhmmmm"*), baixado pelo `install.sh` para `~/.local/share/aic/sounds/plankton-gemido.mp3`. Se o download falhar, escolha um som sintetizado em Config → Som.

## Sprites próprios

Quer o seu próprio Plankton animado? Coloque sprites em `~/.local/share/aic/temas/plankton/sprites/<estado>.(gif|png)` (estados: `boot`, `working`, `idle`, `waiting`, `done`, `gameover`). O que faltar cai no desenho vetorial.

## Trocar de tema

Pela paleta (*Trocar de tema*), em Config → Geral, ou ao abrir: `aic --tema plankton`.

Quer criar outro tema? Copie `crates/aic-app/src/skins/template.rs`, ajuste e registre em `skins::all()`.
