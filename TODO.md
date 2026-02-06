# TO-DO

- [x] Feat: Add a title for `Daemon`.
- [x] Bugfix: If the user copies another wallet address during the `PromptWindow` pop-up, the countdown timer for automatically closing won't be reset. (it should reset!)
- [x] Bugfix: Should immediately return evaluation result while address information is still being fetched. After the fetching thread got the evaluation result, re-render the `PromptWindow`.
- [x] Bugfix: The auto-closing mechanism for `PromptWindow` should be paused when the address information is still being fetched.
- [ ] Bugfix: 如果用戶正在進行編輯, 則不應該自動關閉 `PromptWindow`
- [ ] Bugfix: 如果使用者還在 `🔍 Checking Label...` 的階段就主動關閉了視窗, 則 `PromptWindow` 還會再被跳出來一次。這會微影響 UX。
- [ ] Feat: Add an icon for windows binary.
- [ ] Feat: Makes UI is i18n language supported.
- [ ] Feat: Add a system tray would be a good idea.
- [ ] Feat: 如果地址是個 Smart Contract, 則 Icon 應該用一個 Smart Contract Icon 在原本的 Network Icon 右下角。
- [ ] 