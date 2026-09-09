;;; minitask.el --- Markdown task manager frontend -*- lexical-binding: t; -*-

;; Version: 0.3.0
;; Package-Requires: ((emacs "27.1"))
;; Keywords: tools, convenience

;;; Commentary:
;; M-x minitask opens the shared minitask store through its JSON CLI.
;; No additional Emacs packages are required.  See ../docs/emacs.md.

;;; Code:
(require 'cl-lib)
(require 'json)
(require 'subr-x)
(require 'tabulated-list)
(require 'calendar)
(declare-function org-read-date "org")
(defvar w32-ansi-code-page)
(defvar displayed-month)
(defvar displayed-year)

(defgroup minitask nil "Markdown task management." :group 'tools)
(defconst minitask--directory
  (file-name-directory (or load-file-name buffer-file-name)))
(defcustom minitask-executable
  (or (executable-find "minitask")
      (let ((local (expand-file-name "../minitask.exe" minitask--directory)))
        (if (file-exists-p local) local "minitask")))
  "Path to the minitask executable, without shell arguments."
  :type 'string :group 'minitask)
(defcustom minitask-home nil
  "Store directory.  Nil uses the CLI's environment and platform default."
  :type '(choice (const nil) directory) :group 'minitask)
(defcustom minitask-workspace nil "Initially selected workspace."
  :type '(choice (const nil) string) :group 'minitask)
(defcustom minitask-locale "ja" "Frontend locale name."
  :type 'string :group 'minitask)
(defcustom minitask-locale-directory (expand-file-name "locales" minitask--directory)
  "Directory containing frontend locale JSON files."
  :type 'directory :group 'minitask)
(defvar minitask--strings nil)
(defvar-local minitask--view "resume")
(defvar-local minitask--query "")
(defvar-local minitask--date nil)
(defvar-local minitask--items nil)
(defvar-local minitask--owner nil)
(defvar-local minitask--source-kind nil)
(defvar-local minitask--calendar-items nil)

(defun minitask--json-file (path)
  (with-temp-buffer
    (insert-file-contents path)
    (json-parse-buffer :object-type 'hash-table)))

(defun minitask--text (key &rest args)
  (unless minitask--strings
    (setq minitask--strings
          (minitask--json-file (expand-file-name "locales/ja.json" minitask--directory)))
    (unless (string-match-p "\\`[A-Za-z0-9_-]+\\'" minitask-locale)
      (user-error "%s" (gethash "locale.invalid" minitask--strings)))
    (let ((file (expand-file-name (concat minitask-locale ".json") minitask-locale-directory)))
      (if (file-exists-p file)
          (maphash (lambda (name value)
                     (unless (and (gethash name minitask--strings) (stringp value))
                       (user-error "%s: %s" (gethash "locale.invalid" minitask--strings) name))
                     (puthash name value minitask--strings))
                   (minitask--json-file file))
        (unless (equal minitask-locale "ja")
          (user-error "%s: %s" (gethash "locale.missing" minitask--strings) file)))))
  (apply #'format (or (gethash key minitask--strings) key) args))

(defun minitask--call (&rest args)
  "Call the CLI with ARGS.  Return JSON only after a successful exit."
  (when (and minitask-home (file-remote-p minitask-home))
    (user-error "%s" (minitask--text "error.local")))
  (let* ((executable minitask-executable)
         (args (append (when minitask-home (list "--home" (expand-file-name minitask-home))) args))
         (stderr (make-temp-file "minitask-stderr-"))
        (coding-system-for-read 'utf-8-unix)
        ;; Windows Emacs decodes command-line bytes with the system ANSI page.
        ;; JSON stdout remains UTF-8; using UTF-8 for argv corrupts Japanese.
        (coding-system-for-write
         (if (eq system-type 'windows-nt)
             (if (= w32-ansi-code-page 65001) 'utf-8-unix
               (intern (format "cp%d" w32-ansi-code-page)))
           'utf-8-unix))
        (default-directory minitask--directory))
    (unwind-protect
        (with-temp-buffer
          (dolist (arg (cons executable args))
            (unless (equal arg (decode-coding-string (encode-coding-string arg coding-system-for-write)
                                                    coding-system-for-write))
              (user-error "%s" (minitask--text "error.encoding"))))
          (let ((exit (apply #'process-file executable nil (list t stderr) nil args)))
            (unless (eq exit 0)
              (let ((message (with-temp-buffer (insert-file-contents stderr) (string-trim (buffer-string)))))
                (user-error "%s" (if (string-empty-p message) (minitask--text "error.cli" exit) message))))
            (goto-char (point-min))
            (let ((data (condition-case nil
                            (json-parse-buffer :object-type 'alist :array-type 'list
                                               :null-object nil :false-object nil)
                          (error (user-error "%s" (minitask--text "error.json"))))))
              (unless (eq (alist-get 'schema_version data) 2)
                (user-error "%s" (minitask--text "error.schema")))
              data)))
      (delete-file stderr))))

(defun minitask--scope ()
  (when minitask-workspace (list "--workspace" minitask-workspace)))

(defun minitask--query-items (command)
  (alist-get 'items (apply #'minitask--call command "--full" (minitask--scope))))

(defun minitask--on-date-p (item date)
  (and (string-empty-p (or (alist-get 'validation_error item) ""))
       (member date (mapcar (lambda (field) (alist-get field item)) '(scheduled due date)))))

(defun minitask--fetch ()
  (let ((items
         (cond
          ((equal minitask--view "resume")
           (let ((data (apply #'minitask--call "resume" "--full" (minitask--scope))))
             (apply #'append (mapcar (lambda (key) (alist-get key data))
                                    '(issues actions directions questions expectations notes)))))
          ((equal minitask--view "date")
           (cl-remove-if-not
            (lambda (item) (minitask--on-date-p item minitask--date))
            (append (minitask--query-items "tasks") (minitask--query-items "expectations"))))
          (t (minitask--query-items minitask--view)))))
    (if (string-empty-p minitask--query) items
      (cl-remove-if-not
       (lambda (item)
         (string-match-p (regexp-quote (downcase minitask--query))
                         (downcase (format "%s\n%s" (alist-get 'title item) (alist-get 'notes item))))) items))))

(defun minitask--status-label (item)
  (if (not (string-empty-p (or (alist-get 'validation_error item) "")))
      (minitask--text "state.error")
    (minitask--text (if (and (equal (alist-get 'kind item) "expectation")
                            (equal (alist-get 'status item) "open"))
                       "state.expected"
                     (concat "state." (alist-get 'status item))))))

(defun minitask--row (item)
  (let* ((status (alist-get 'status item))
         (face (cond ((not (string-empty-p (or (alist-get 'validation_error item) ""))) 'error)
                     ((equal status "in_progress") 'font-lock-keyword-face)
                     ((member status '("completed" "achieved" "answered")) 'success)
                     ((equal status "blocked") 'warning)
                     (t 'default))))
    (list (alist-get 'id item)
          (vector (propertize (minitask--status-label item) 'face face)
                  (minitask--text (concat "kind." (alist-get 'kind item)))
                  (replace-regexp-in-string "[\n\r\t]" " " (alist-get 'title item))
                  (or (alist-get 'scheduled item) (alist-get 'date item) "")
                  (or (alist-get 'due item) "")))))

(defun minitask-refresh ()
  "Refresh this list, retaining the selected item by ID."
  (interactive)
  (setq minitask--strings nil)
  (minitask--columns)
  (let ((items (minitask--fetch)))
    (setq minitask--items items
          tabulated-list-entries (mapcar #'minitask--row items)
          header-line-format
          (minitask--text "header" (or minitask-workspace (minitask--text "workspace.none"))
                          (if (equal minitask--view "date") minitask--date
                            (minitask--text (concat "view." minitask--view)))
                          (length items) minitask--query))
    (tabulated-list-print t)
    (when (and items (not (tabulated-list-get-id)))
      (goto-char (point-min))
      (forward-line 1))))

(defun minitask--selected ()
  (or (cl-find (tabulated-list-get-id) minitask--items :key (lambda (item) (alist-get 'id item)) :test #'equal)
      (user-error "%s" (minitask--text "error.selection"))))

(defun minitask--mutate (item command &rest args)
  "Update ITEM through COMMAND with ARGS, preserving stale/dirty data guards."
  (let* ((path (alist-get 'path item))
         (buffer (get-file-buffer path))
         (current (alist-get 'item (minitask--call "show" "--id" (alist-get 'id item)))))
    (when (and buffer (buffer-modified-p buffer))
      (user-error "%s" (minitask--text "error.unsaved")))
    (unless (equal (alist-get 'hash item) (alist-get 'hash current))
      (user-error "%s" (minitask--text "error.stale")))
    (prog1 (apply #'minitask--call command "--id" (alist-get 'id current)
                  "--expected-hash" (alist-get 'hash current) args)
      (when (buffer-live-p buffer)
        (with-current-buffer buffer (revert-buffer t t t))))))

(defun minitask-switch-view ()
  "Select a view."
  (interactive)
  (let* ((choices (mapcar (lambda (name) (cons (minitask--text (concat "view." name)) name))
                          '("resume" "tasks" "expectations" "notes" "directions" "questions")))
         (choice (completing-read (minitask--text "prompt.view") choices nil t)))
    (setq minitask--view (cdr (assoc choice choices)) minitask--query "")
    (minitask-refresh)))

(defun minitask-switch-workspace ()
  "Select a workspace from the actual store."
  (interactive)
  (let* ((data (minitask--call "workspaces"))
         (names (mapcar (lambda (item) (alist-get 'name item)) (alist-get 'items data))))
    (setq-local minitask-home (alist-get 'home data))
    (unless names (user-error "%s" (minitask--text "workspace.empty")))
    (setq-local minitask-workspace (completing-read (minitask--text "prompt.workspace") names nil t))
    (minitask-refresh)))

(defun minitask-search (query)
  "Filter this view by QUERY in title and details."
  (interactive (list (read-string (minitask--text "prompt.search") minitask--query)))
  (setq minitask--query query)
  (minitask-refresh))

(defun minitask-show ()
  "Display the selected item's details alongside the list."
  (interactive)
  (let* ((item (alist-get 'item (minitask--call "show" "--id" (alist-get 'id (minitask--selected)))))
         (buffer (get-buffer-create "*minitask-detail*")))
    (with-current-buffer buffer
      (let ((inhibit-read-only t))
        (erase-buffer)
        (insert (propertize (alist-get 'title item) 'face 'bold) "\n\n")
        (insert (minitask--text "detail.state" (minitask--text (concat "kind." (alist-get 'kind item)))
                                (minitask--status-label item)) "\n")
        (insert (minitask--text "detail.dates" (or (alist-get 'scheduled item) (alist-get 'date item) "-")
                                (or (alist-get 'due item) "-")) "\n")
        (insert (alist-get 'path item) "\n\n")
        (unless (string-empty-p (or (alist-get 'validation_error item) ""))
          (insert (propertize (alist-get 'validation_error item) 'face 'error) "\n\n"))
        (let ((body (or (alist-get 'notes item) "")))
          (unless (equal (alist-get 'kind item) "note")
            (dolist (heading '("Completion Condition" "Supplement" "Answer" "Verification"))
              (setq body (replace-regexp-in-string
                          (concat "^## " heading "$")
                          (minitask--text (concat "heading." heading)) body t t))))
          (insert body))
        (goto-char (point-min))
        (special-mode)
        (setq-local mode-name (minitask--text "detail.mode"))
        (visual-line-mode 1)))
    (display-buffer buffer '(display-buffer-pop-up-window))))

(defun minitask--after-save ()
  (when (buffer-live-p minitask--owner)
    (with-current-buffer minitask--owner
      (condition-case err (minitask-refresh) (error (message "%s" (error-message-string err)))))))

(defun minitask-edit-finish ()
  "Save the Markdown and return to its task list."
  (interactive)
  (save-buffer)
  (if (buffer-live-p minitask--owner) (pop-to-buffer minitask--owner)
    (minitask)))

(define-minor-mode minitask-edit-mode
  "Return with C-c C-c; create a task from a note region with C-c C-t."
  :lighter " minitask"
  :keymap (let ((map (make-sparse-keymap)))
            (define-key map (kbd "C-c C-c") #'minitask-edit-finish)
            (define-key map (kbd "C-c C-t") #'minitask-region-to-task)
            map))

(defun minitask--edit-item (item)
  (let ((owner (if (derived-mode-p 'minitask-mode) (current-buffer) minitask--owner))
        (home minitask-home) (executable minitask-executable))
    (find-file-other-window (alist-get 'path item))
    (goto-char (point-min))
    (forward-line (1- (alist-get 'line item)))
    (setq-local minitask--owner owner minitask-home home minitask-executable executable
                minitask-workspace (alist-get 'workspace item) minitask--source-kind (alist-get 'kind item))
    (minitask-edit-mode 1)
    (add-hook 'after-save-hook #'minitask--after-save nil t)
    (message "%s" (minitask--text "edit.hint"))))

(defun minitask-region-to-task (begin end)
  "Copy the selected note text to a new task in the note's workspace."
  (interactive
   (progn
     (unless (use-region-p) (user-error "%s" (minitask--text "error.region")))
     (list (region-beginning) (region-end))))
  (unless (and minitask-edit-mode (equal minitask--source-kind "note") minitask-workspace)
    (user-error "%s" (minitask--text "error.note-region")))
  (let ((body (buffer-substring-no-properties begin end)))
    (when (string-empty-p (string-trim body))
      (user-error "%s" (minitask--text "error.region")))
    (let* ((title (read-string (minitask--text "prompt.title")
                               (car (split-string (string-trim body) "[\r\n]+" t))))
           (condition (read-string (minitask--text "prompt.condition")))
           (date (format-time-string "%Y-%m-%d"))
           (item (alist-get 'item
                           (minitask--call "add" "--workspace" minitask-workspace "--kind" "action"
                                           "--title" (format "%s [scheduled:: %s] [due:: %s]" title date date)
                                           "--completion-condition" condition "--body" body))))
      (deactivate-mark)
      (minitask--after-save)
      (minitask--edit-item item))))

(defun minitask-edit ()
  "Open the selected Markdown using the user's normal major mode."
  (interactive)
  (minitask--edit-item (alist-get 'item (minitask--call "show" "--id" (alist-get 'id (minitask--selected))))))

(defun minitask-set-status ()
  "Choose an allowed transition and record required evidence."
  (interactive)
  (let* ((item (minitask--selected))
         (choices (mapcar (lambda (status)
                           (cons (minitask--status-label (cons (cons 'status status)
                                                              (assq-delete-all 'status (copy-alist item)))) status))
                         (alist-get 'allowed_transitions item))))
    (unless choices (user-error "%s" (minitask--text "error.no-transition")))
    (let* ((status (cdr (assoc (completing-read (minitask--text "prompt.status") choices nil t) choices)))
           (kind (alist-get 'kind item))
           (evidence
            (cond
             ((and (equal kind "action") (equal status "completed"))
              (when (string-empty-p (or (alist-get 'completion_condition item) ""))
                (user-error "%s" (minitask--text "error.condition")))
              (unless (yes-or-no-p (minitask--text "prompt.verified" (alist-get 'completion_condition item)))
                (user-error "%s" (minitask--text "cancelled")))
              (read-string (minitask--text "prompt.evidence")))
             ((and (equal kind "question") (equal status "answered"))
              (read-string (minitask--text "prompt.answer"))))))
      (apply #'minitask--mutate item "set-status" "--status" status
             (when evidence (list "--evidence" evidence)))
      (minitask-refresh))))

(defun minitask--read-date (prompt &optional initial)
  (require 'org)
  (let ((calendar-month-name-array (vconcat (split-string (minitask--text "calendar.months") "|")))
        (calendar-month-abbrev-array (vconcat (split-string (minitask--text "calendar.months") "|")))
        (calendar-day-name-array (vconcat (split-string (minitask--text "calendar.days") "|")))
        (calendar-day-abbrev-array (vconcat (split-string (minitask--text "calendar.short-days") "|")))
        (calendar-day-header-array (vconcat (split-string (minitask--text "calendar.short-days") "|")))
        (calendar-week-start-day 1))
    (org-read-date nil nil nil prompt
                   (when initial (date-to-time (concat initial " 00:00:00"))))))

(defun minitask-set-date ()
  "Edit a task date or a milestone date using the calendar picker."
  (interactive)
  (let* ((item (minitask--selected))
         (kind (alist-get 'kind item))
         (field (cond ((equal kind "expectation") "expectation")
                      ((equal kind "action")
                       (let ((choices (list (cons (minitask--text "column.scheduled") "scheduled")
                                            (cons (minitask--text "column.due") "due"))))
                         (cdr (assoc (completing-read (minitask--text "prompt.date-field") choices nil t) choices))))
                      (t (user-error "%s" (minitask--text "error.date-kind")))))
         (old (alist-get (if (equal field "expectation") 'date (intern field)) item))
         (clear (and old (equal kind "action")
                     (y-or-n-p (minitask--text "prompt.clear-date"))))
         (date (if clear "none" (minitask--read-date (minitask--text "prompt.date") old))))
    (minitask--mutate item "set-date" "--field" field "--date" date)
    (minitask-refresh)))

(defun minitask-add ()
  "Create an entity and open its Markdown for details."
  (interactive)
  (unless minitask-workspace (user-error "%s" (minitask--text "workspace.empty")))
  (let* ((choices (mapcar (lambda (kind) (cons (minitask--text (concat "kind." kind)) kind))
                          '("action" "note" "direction" "question" "expectation")))
         (default (or (cdr (assoc minitask--view '(("notes" . "note") ("directions" . "direction")
                                                 ("questions" . "question") ("expectations" . "expectation")))) "action"))
         (kind (cdr (assoc (completing-read (minitask--text "prompt.kind") choices nil t nil nil
                                           (minitask--text (concat "kind." default))) choices)))
         (title (read-string (minitask--text "prompt.title")))
         (date (or (and (equal minitask--view "date") minitask--date)
                   (format-time-string "%Y-%m-%d")))
         (args (append (list "add" "--workspace" minitask-workspace "--kind" kind "--title"
                             (if (equal kind "action")
                                 (format "%s [scheduled:: %s] [due:: %s]" title date date)
                               title))
                       (when (equal kind "action")
                         (list "--completion-condition" (read-string (minitask--text "prompt.condition"))))
                       (when (equal kind "expectation")
                         (list "--date" (minitask--read-date (minitask--text "prompt.date") minitask--date)))))
         (item (alist-get 'item (apply #'minitask--call args))))
    (minitask-refresh)
    (minitask--edit-item item)))

(defun minitask-create-workspace (name)
  "Create workspace NAME as a directory directly below the store."
  (interactive (list (read-string (minitask--text "prompt.new-workspace"))))
  (unless (and (not (string-empty-p name)) (equal name (string-trim name))
               (not (string-prefix-p "." name))
               (not (string-match-p "[/\\\\<>:\"|?*\x00-\x1f]" name))
               (not (string-suffix-p "." name)))
    (user-error "%s" (minitask--text "error.workspace-name")))
  (let ((root (alist-get 'home (minitask--call "workspaces"))))
    (make-directory (expand-file-name name root))
    (setq-local minitask-home root minitask-workspace name)
    (minitask-refresh)))

(defun minitask--calendar-mark ()
  (dolist (item minitask--calendar-items)
    (when (string-empty-p (or (alist-get 'validation_error item) ""))
      (dolist (field '(scheduled due date))
        (when-let* ((date (alist-get field item)) (parts (mapcar #'string-to-number (split-string date "-"))))
          (let ((day (list (nth 1 parts) (nth 2 parts) (car parts))))
            (when (calendar-date-is-visible-p day) (calendar-mark-visible-date day 'font-lock-keyword-face))))))))

(defun minitask-calendar-open-day ()
  "Show items for the calendar day at point."
  (interactive)
  (let* ((day (calendar-cursor-to-date t))
         (date (format "%04d-%02d-%02d" (nth 2 day) (car day) (nth 1 day)))
         (owner minitask--owner))
    (unless (buffer-live-p owner) (user-error "%s" (minitask--text "error.owner")))
    (pop-to-buffer owner)
    (setq minitask--date date minitask--view "date" minitask--query "")
    (minitask-refresh)))

(defun minitask-calendar ()
  "Open a marked calendar.  RET shows tasks and milestones for a day."
  (interactive)
  (let ((owner (current-buffer))
        (items (append (minitask--query-items "tasks") (minitask--query-items "expectations"))))
    (calendar)
    (setq-local minitask--owner owner minitask--calendar-items items
                calendar-month-name-array (vconcat (split-string (minitask--text "calendar.months") "|"))
                calendar-month-abbrev-array (vconcat (split-string (minitask--text "calendar.months") "|"))
                calendar-day-name-array (vconcat (split-string (minitask--text "calendar.days") "|"))
                calendar-day-abbrev-array (vconcat (split-string (minitask--text "calendar.short-days") "|"))
                calendar-day-header-array (vconcat (split-string (minitask--text "calendar.short-days") "|"))
                calendar-mode-line-format (list (minitask--text "calendar.mode"))
                calendar-week-start-day 1)
    (use-local-map (copy-keymap (current-local-map)))
    (local-set-key (kbd "RET") #'minitask-calendar-open-day)
    (setq-local header-line-format (minitask--text "calendar.hint"))
    (add-hook 'calendar-today-visible-hook #'minitask--calendar-mark nil t)
    (add-hook 'calendar-today-invisible-hook #'minitask--calendar-mark nil t)
    (calendar-generate-window displayed-month displayed-year)))

(defun minitask-help ()
  "Show the frontend's key bindings."
  (interactive)
  (with-help-window "*minitask-help*" (princ (minitask--text "help"))))

(defvar minitask-mode-map
  (let ((map (make-sparse-keymap)))
    (set-keymap-parent map tabulated-list-mode-map)
    (dolist (pair '(("g" . minitask-refresh) ("w" . minitask-switch-workspace)
                    ("v" . minitask-switch-view) ("/" . minitask-search) ("a" . minitask-add)
                    ("n" . minitask-create-workspace) ("s" . minitask-set-status)
                    ("d" . minitask-set-date) ("c" . minitask-calendar) ("e" . minitask-edit)
                    ("RET" . minitask-show) ("?" . minitask-help)
                    ("j" . next-line) ("k" . previous-line) ("q" . quit-window)))
      (define-key map (kbd (car pair)) (cdr pair)))
    map))

(defun minitask--columns ()
  (setq-local tabulated-list-format
              (vector (list (minitask--text "column.state") 10 t)
                      (list (minitask--text "column.kind") 14 t)
                      (list (minitask--text "column.title") (max 20 (- (window-body-width) 50)) t)
                      (list (minitask--text "column.scheduled") 10 t)
                      (list (minitask--text "column.due") 10 t)))
  (tabulated-list-init-header))

(define-derived-mode minitask-mode tabulated-list-mode "minitask"
  "Browse minitask entities.  See \[minitask-help] for commands."
  (setq-local tabulated-list-use-header-line nil
              tabulated-list-padding 1
              tabulated-list-sort-key nil
              revert-buffer-function (lambda (&rest _) (minitask-refresh)))
  (minitask--columns)
  (hl-line-mode 1))

(with-eval-after-load 'evil
  (when (fboundp 'evil-set-initial-state) (evil-set-initial-state 'minitask-mode 'emacs)))

;;;###autoload
(defun minitask ()
  "Open minitask inside Emacs."
  (interactive)
  (let ((buffer (get-buffer-create "*minitask*"))
        (root minitask-home) (exe minitask-executable) (workspace minitask-workspace))
    (pop-to-buffer buffer)
    (unless (derived-mode-p 'minitask-mode)
      (minitask-mode)
      (setq-local minitask-home root minitask-executable exe minitask-workspace workspace))
    (let* ((data (minitask--call "workspaces"))
           (names (mapcar (lambda (item) (alist-get 'name item)) (alist-get 'items data))))
      (setq-local minitask-home (alist-get 'home data))
      (unless (member minitask-workspace names)
        (setq-local minitask-workspace
                    (if (> (length names) 1)
                        (completing-read (minitask--text "prompt.workspace") names nil t)
                      (car names)))))
    (minitask-refresh)
    (message "%s" (minitask--text "hint"))))

(provide 'minitask)
;;; minitask.el ends here
