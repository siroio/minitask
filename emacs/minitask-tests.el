;;; minitask-tests.el --- Integration tests -*- lexical-binding: t; -*-
(require 'ert)
(require 'cl-lib)
(setq load-prefer-newer t)
(require 'minitask)

(defconst minitask-test-executable
  (expand-file-name "../minitask.exe" (file-name-directory (or load-file-name buffer-file-name))))

(defmacro minitask-test-with-store (&rest body)
  `(let* ((test-root (expand-file-name ".cache" (file-name-directory minitask-test-executable)))
          (minitask-home (make-temp-file (expand-file-name "minitask-ert-" test-root) t))
          (minitask-executable minitask-test-executable)
          (minitask-workspace "開発")
          (minitask--strings nil))
     (unwind-protect
         (progn
           (make-directory (expand-file-name "開発" minitask-home))
           (make-directory (expand-file-name "別案件" minitask-home))
           ,@body)
       (dolist (buffer (buffer-list))
         (when (and (buffer-file-name buffer)
                    (file-in-directory-p (buffer-file-name buffer) minitask-home))
           (with-current-buffer buffer (set-buffer-modified-p nil))
           (kill-buffer buffer)))
       (unless (file-in-directory-p minitask-home test-root) (error "Unexpected fixture path"))
       (delete-directory minitask-home t))))

(defun minitask-test-add (kind title &rest args)
  (alist-get 'item (apply #'minitask--call "add" "--workspace" "開発"
                         "--kind" kind "--title" title args)))

(ert-deftest minitask-cli-japanese-and-read-errors ()
  (minitask-test-with-store
   (let ((item (minitask-test-add "note" "日本語のメモ" "--body" "自由な本文\n- [ ] メモ")))
     (should (equal "日本語のメモ" (alist-get 'title item)))
     (should (equal "note" (alist-get 'kind item)))
     (should-error (minitask--call "show" "--id" "missing") :type 'user-error)
     (should (equal "作業の続き" (minitask--text "view.resume"))))))

(ert-deftest minitask-list-scope-search-and-stable-id ()
  (minitask-test-with-store
   (let ((item (minitask-test-add "action" "表示を確認" "--completion-condition" "表示される"))
         (root minitask-home) (exe minitask-executable))
     (minitask-test-add "expectation" "初回デモ" "--date" "2026-09-30")
     (minitask--call "add" "--workspace" "別案件" "--kind" "action" "--title" "表示を確認")
     (with-temp-buffer
       (minitask-mode)
       (setq-local minitask-home root minitask-executable exe minitask-workspace "開発")
       (minitask-refresh)
       (should (= 2 (length tabulated-list-entries)))
       (should (string-match-p "タイトル" (buffer-string)))
       (should (assoc (alist-get 'id item) tabulated-list-entries))
       (setq minitask--query "デモ")
       (minitask-refresh)
       (should (= 1 (length tabulated-list-entries)))
       (setq minitask--query "" minitask--view "tasks")
       (minitask-refresh)
       (should (= 1 (length tabulated-list-entries)))))))

(ert-deftest minitask-guarded-mutations-and-completion ()
  (minitask-test-with-store
   (let* ((item (minitask-test-add "action" "検証" "--completion-condition" "読み取りが成功"))
          (id (alist-get 'id item)))
     (should-error (minitask--mutate item "set-status" "--status" "completed") :type 'user-error)
     (minitask--mutate item "set-status" "--status" "in_progress")
     (should-error (minitask--mutate item "set-status" "--status" "completed" "--evidence" "確認") :type 'user-error)
     (setq item (alist-get 'item (minitask--call "show" "--id" id)))
     (let ((buffer (find-file-noselect (alist-get 'path item))))
       (with-current-buffer buffer (goto-char (point-max)) (insert "未保存"))
       (should-error (minitask--mutate item "set-status" "--status" "completed" "--evidence" "確認") :type 'user-error)
       (with-current-buffer buffer (set-buffer-modified-p nil))
       (kill-buffer buffer))
     (minitask--mutate item "set-status" "--status" "completed" "--evidence" "showによる本文読み取りを確認")
     (should (equal "completed" (alist-get 'status (alist-get 'item (minitask--call "show" "--id" id))))))))

(ert-deftest minitask-calendar-matches-both-action-dates-and-milestones ()
  (minitask-test-with-store
   (let ((item (minitask-test-add "action" "予定")))
     (minitask--mutate item "set-date" "--field" "scheduled" "--date" "2026-09-30"))
   (minitask-test-add "expectation" "デモ" "--date" "2026-09-30")
   (let ((root minitask-home) (exe minitask-executable))
     (with-temp-buffer
       (minitask-mode)
       (setq-local minitask-home root minitask-executable exe minitask-workspace "開発"
                   minitask--view "date" minitask--date "2026-09-30")
       (minitask-refresh)
       (should (= 2 (length tabulated-list-entries)))))))

(ert-deftest minitask-locale-partial-override ()
  (let* ((minitask-locale-directory (make-temp-file "minitask-locale-" t))
         (minitask-locale "test") (minitask--strings nil))
    (unwind-protect
        (progn
          (with-temp-file (expand-file-name "test.json" minitask-locale-directory)
            (insert "{\"view.resume\":\"Continue\"}"))
          (should (equal "Continue" (minitask--text "view.resume")))
          (should (equal "ノート" (minitask--text "kind.note")) ))
      (delete-directory minitask-locale-directory t))))

(ert-deftest minitask-calendar-navigation-opens-date-view ()
  (minitask-test-with-store
   (minitask-test-add "expectation" "デモ" "--date" "2026-09-30")
   (let ((root minitask-home) (exe minitask-executable))
     (save-window-excursion
       (with-temp-buffer
         (switch-to-buffer (current-buffer))
         (minitask-mode)
         (setq-local minitask-home root minitask-executable exe minitask-workspace "開発")
         (let ((owner (current-buffer)))
           (minitask-calendar)
           (calendar-goto-date '(9 30 2026))
           (should (string-match-p "9月" (buffer-string)))
           (should (string-match-p "月 +火 +水" (buffer-string)))
           (minitask-calendar-open-day)
           (should (eq (current-buffer) owner))
           (should (equal minitask--date "2026-09-30"))
           (should (= 1 (length tabulated-list-entries)))))))))

(ert-deftest minitask-edit-save-refresh-and-return ()
  (minitask-test-with-store
   (let ((item (minitask-test-add "action" "詳細の検証" "--completion-condition" "本文が更新される"))
         (root minitask-home) (exe minitask-executable))
     (save-window-excursion
       (with-temp-buffer
         (switch-to-buffer (current-buffer))
         (minitask-mode)
         (setq-local minitask-home root minitask-executable exe minitask-workspace "開発")
         (minitask-refresh)
         (should (equal (alist-get 'id item) (tabulated-list-get-id)))
         (minitask-show)
         (with-current-buffer "*minitask-detail*"
           (should (string-match-p "## 完了条件" (buffer-string))))
         (let ((owner (current-buffer)))
           (minitask-edit)
           (should minitask-edit-mode)
           (goto-char (point-max))
           (insert "\n保存後のメモ\n")
           (minitask-edit-finish)
           (should (eq (current-buffer) owner))
           (should (string-match-p "保存後のメモ" (alist-get 'notes (car minitask--items))))
           (minitask--mutate (car minitask--items) "set-status" "--status" "in_progress")
           (with-current-buffer (get-file-buffer (alist-get 'path item))
             (should minitask-edit-mode)
             (should (eq minitask--owner owner))
             (should (memq #'minitask--after-save after-save-hook)))))))))

(ert-deftest minitask-add-opens-individual-note ()
  (minitask-test-with-store
   (let ((root minitask-home) (exe minitask-executable))
     (save-window-excursion
       (with-temp-buffer
         (switch-to-buffer (current-buffer))
         (minitask-mode)
         (setq-local minitask-home root minitask-executable exe minitask-workspace "開発")
         (cl-letf (((symbol-function 'completing-read) (lambda (&rest _) "ノート"))
                   ((symbol-function 'read-string) (lambda (&rest _) "引用\"と 空白")))
           (minitask-add))
         (should (string-prefix-p "note-" (file-name-nondirectory buffer-file-name)))
         (should (string-match-p "引用\"と 空白" (buffer-string))))))))

(ert-deftest minitask-calendar-picker-replaces-existing-date ()
  (require 'org)
  (let ((read-date (symbol-function 'org-read-date)))
    ;; Org combines the minibuffer input and calendar selection in this order.
    ;; Exercise its real parser without opening an interactive minibuffer.
    (cl-letf (((symbol-function 'org-read-date)
               (lambda (_time _to _from prompt default-time &optional initial)
                 (funcall read-date nil nil (concat initial " 2026-10-05") prompt default-time))))
      (should (equal "2026-10-05" (minitask--read-date "日付" "2026-09-30"))))))

(ert-deftest minitask-add-action-defaults-to-one-calendar-day ()
  (minitask-test-with-store
   (let ((root minitask-home) (exe minitask-executable))
     (dolist (selected '(nil "2028-02-29"))
       (save-window-excursion
         (with-temp-buffer
           (switch-to-buffer (current-buffer))
           (minitask-mode)
           (setq-local minitask-home root minitask-executable exe minitask-workspace "開発"
                       minitask--date selected minitask--view (if selected "date" "tasks"))
           (let ((title (if selected "カレンダーから追加" "今日の作業")))
             (cl-letf (((symbol-function 'completing-read) (lambda (&rest _) "タスク"))
                       ((symbol-function 'read-string) (lambda (&rest _) title)))
               (minitask-add))
             (let* ((items (alist-get 'items (minitask--call "tasks" "--workspace" "開発" "--query" title)))
                    (item (car items))
                    (date (or selected (format-time-string "%Y-%m-%d"))))
               (should (= 1 (length items)))
               (should (equal date (alist-get 'scheduled item)))
               (should (equal date (alist-get 'due item)))))))))))

(ert-deftest minitask-region-task-preserves-unsaved-note-and-its-workspace ()
  (minitask-test-with-store
   (let* ((note (minitask-test-add "note" "作業ノート" "--body" "保存してある文"))
          (root minitask-home) (exe minitask-executable)
          (path (alist-get 'path note))
          (disk (with-temp-buffer (insert-file-contents-literally path) (buffer-string)))
          (body "選択した作業\n引用\"と 空白\n- 詳細メモ\n"))
     (save-window-excursion
       (with-temp-buffer
         (switch-to-buffer (current-buffer))
         (minitask-mode)
         (setq-local minitask-home root minitask-executable exe minitask-workspace "開発")
         (let ((owner (current-buffer)))
           (minitask--edit-item note)
           (with-current-buffer owner (setq minitask-workspace "別案件"))
           (goto-char (point-max))
           (insert "\n未選択の文\n")
           (let ((start (point)) (source (current-buffer)))
             (insert body)
             (set-mark start)
             (activate-mark)
             (let ((before (buffer-string)) (transient-mark-mode t))
               ;; Different ambient settings must not override the edit buffer.
               (with-temp-buffer
                 (let ((minitask-home (expand-file-name "別案件" root))
                       (minitask-executable "nonexistent-minitask"))
                   (cl-letf (((symbol-function 'read-string)
                              (lambda (prompt &optional initial &rest _)
                                (if (equal prompt (minitask--text "prompt.title"))
                                    (progn (should (equal initial "選択した作業")) "切り出した作業")
                                  "検証が通る"))))
                     (with-current-buffer source
                       (call-interactively #'minitask-region-to-task)))))
               (should (equal (key-binding (kbd "C-c C-t")) #'minitask-region-to-task))
               (should (eq minitask--owner owner))
               (should (equal minitask-workspace "開発"))
               (with-current-buffer source
                 (should (equal before (buffer-string)))
                 (should (buffer-modified-p)))
               (should (equal disk (with-temp-buffer (insert-file-contents-literally path) (buffer-string))))
               (let* ((minitask-home root) (minitask-executable exe)
                      (items (alist-get 'items (minitask--call "tasks" "--workspace" "開発" "--full")))
                      (task (car items)))
                 (should (= 1 (length items)))
                 (should (equal (alist-get 'title task) "切り出した作業"))
                 (with-current-buffer (get-file-buffer (alist-get 'path task))
                   (should (eq minitask--owner owner))
                   (should (equal minitask-workspace "開発"))
                   (should (equal minitask-home root)))
                 (should (string-match-p (regexp-quote body)
                                        (with-temp-buffer
                                          (insert-file-contents (alist-get 'path task)) (buffer-string))))
                 (should (equal (alist-get 'completion_condition task) "検証が通る"))
                 (should (equal (alist-get 'scheduled task) (format-time-string "%Y-%m-%d")))
                 (should (equal (alist-get 'scheduled task) (alist-get 'due task)))
                 (should-not (alist-get 'items (minitask--call "tasks" "--workspace" "別案件"))))))))))))

(ert-deftest minitask-region-task-rejects-empty-selection-and-preserves-note-on-failure ()
  (minitask-test-with-store
   (let ((note (minitask-test-add "note" "元のノート" "--body" "切り出す文章"))
         (root minitask-home) (exe minitask-executable))
     (save-window-excursion
       (with-temp-buffer
         (switch-to-buffer (current-buffer))
         (minitask-mode)
         (setq-local minitask-home root minitask-executable exe minitask-workspace "開発")
         (minitask--edit-item note)
         (let ((before (buffer-string)) (source (current-buffer)))
           (deactivate-mark)
           (should-error (call-interactively #'minitask-region-to-task) :type 'user-error)
           (should-error (minitask-region-to-task (point-min) (point-min)) :type 'user-error)
           (cl-letf (((symbol-function 'read-string) (lambda (&rest _) (signal 'quit nil))))
             (should (eq 'cancelled
                         (condition-case nil
                             (minitask-region-to-task (point-min) (point-max))
                           (quit 'cancelled)))))
           ;; A blank title is rejected by the actual CLI, without editing the note.
           (cl-letf (((symbol-function 'read-string) (lambda (&rest _) "")))
             (should-error (minitask-region-to-task (point-min) (point-max)) :type 'user-error))
           (should (eq (current-buffer) source))
           (should (equal before (buffer-string)))
           (should-not (buffer-modified-p))
           (should-not (alist-get 'items (minitask--call "tasks" "--workspace" "開発")))))))))

;;; minitask-tests.el ends here
