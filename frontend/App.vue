<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

type Mode = "replace" | "insert";

interface ChangeRequest {
  dir: string;
  mode: Mode;
  find: string;
  replaceWith: string;
  after: string;
  insertText: string;
}

interface DiffResponse {
  lineNo: number;
  old: string;
  new: string;
}

interface FileResponse {
  path: string;
  diffs: DiffResponse[];
}

interface ChangeResponse {
  filesScanned: number;
  filesModified: number;
  totalEdits: number;
  binarySkipped: number;
  details: FileResponse[];
  errors: { path: string; message: string }[];
}

const mode = ref<Mode>("replace");
const directory = ref("");
const find = ref("");
const replaceWith = ref("");
const after = ref("");
const insertText = ref("");
const preview = ref<ChangeResponse | null>(null);
const busy = ref(false);
const errorMessage = ref("");

const canPreview = computed(() => Boolean(directory.value.trim()) &&
  (mode.value === "replace" ? Boolean(find.value) : Boolean(after.value)));

const emptyState = computed(() => {
  if (errorMessage.value) {
    return { title: "操作失败", body: errorMessage.value, note: "请检查目录和操作参数" };
  }
  if (preview.value) {
    return {
      title: "没有发现可修改内容",
      body: `已扫描 ${preview.value.filesScanned} 个文件，没有匹配到需要处理的文本。`,
      note: "请调整查找文本后再次预览",
    };
  }
  return {
    title: "从左侧开始设置",
    body: "选择工作目录和操作模式，预览结果会显示在这里。",
    note: "预览不会修改任何文件",
  };
});

const request = computed<ChangeRequest>(() => ({
  dir: directory.value.trim(),
  mode: mode.value,
  find: find.value,
  replaceWith: replaceWith.value,
  after: after.value,
  insertText: insertText.value,
}));

function clearPreview(): void {
  preview.value = null;
  errorMessage.value = "";
}

function selectMode(nextMode: Mode): void {
  mode.value = nextMode;
  clearPreview();
}

function handleInput(): void {
  if (preview.value || errorMessage.value) clearPreview();
}

async function saveDirectory(): Promise<void> {
  if (!directory.value.trim()) return;
  try {
    await invoke("save_directory", { dir: directory.value.trim() });
  } catch (error: unknown) {
    console.error(error);
  }
}

async function chooseDirectory(): Promise<void> {
  try {
    const selected = await open({ directory: true, multiple: false, title: "选择工作目录" });
    if (typeof selected !== "string") return;
    directory.value = selected;
    clearPreview();
    await saveDirectory();
  } catch (error: unknown) {
    errorMessage.value = String(error);
  }
}

async function runPreview(): Promise<void> {
  if (!canPreview.value || busy.value) return;
  busy.value = true;
  errorMessage.value = "";
  try {
    preview.value = await invoke<ChangeResponse>("preview_changes", { request: request.value });
  } catch (error: unknown) {
    preview.value = null;
    errorMessage.value = String(error);
  } finally {
    busy.value = false;
  }
}

async function applyChanges(): Promise<void> {
  if (!preview.value || !canPreview.value || busy.value) return;
  busy.value = true;
  errorMessage.value = "";
  try {
    preview.value = await invoke<ChangeResponse>("apply_changes", { request: request.value });
  } catch (error: unknown) {
    preview.value = null;
    errorMessage.value = String(error);
  } finally {
    busy.value = false;
  }
}

onMounted(async () => {
  try {
    const saved = await invoke<string | null>("load_saved_dir");
    if (saved) directory.value = saved;
  } catch (error: unknown) {
    console.error(error);
  }
});
</script>

<template>
  <div class="app-shell">
    <main class="workspace">
      <aside class="sidebar">
        <div class="sidebar-scroll">
          <h1>编辑规则</h1>

          <section class="control-section">
            <div class="section-title">工作目录</div>
            <div class="directory-card surface-card">
              <div class="field-heading">
                <span>目录</span>
                <button class="quiet-button" type="button" :disabled="busy" @click="chooseDirectory">选择</button>
              </div>
              <input
                v-model="directory"
                class="text-input"
                type="text"
                placeholder="选择要处理的文件夹"
                autocomplete="off"
                :disabled="busy"
                @input="handleInput"
                @blur="saveDirectory"
              />
            </div>
          </section>

          <section class="control-section">
            <div class="section-title">操作模式</div>
            <div class="segmented-control" role="tablist" aria-label="操作模式">
              <button
                class="segment"
                :class="{ 'is-selected': mode === 'replace' }"
                type="button"
                role="tab"
                :aria-selected="mode === 'replace'"
                :disabled="busy"
                @click="selectMode('replace')"
              >替换</button>
              <button
                class="segment"
                :class="{ 'is-selected': mode === 'insert' }"
                type="button"
                role="tab"
                :aria-selected="mode === 'insert'"
                :disabled="busy"
                @click="selectMode('insert')"
              >插入</button>
            </div>
          </section>

          <section v-if="mode === 'replace'" class="control-section operation-fields">
            <div class="section-title">查找与替换</div>
            <label class="field-label" for="findInput">查找文本</label>
            <textarea
              id="findInput"
              v-model="find"
              class="text-input text-area"
              rows="3"
              placeholder="输入要查找的内容"
              :disabled="busy"
              @input="handleInput"
            ></textarea>
            <label class="field-label" for="replaceInput">替换为</label>
            <textarea
              id="replaceInput"
              v-model="replaceWith"
              class="text-input text-area"
              rows="3"
              placeholder="输入替换后的内容"
              :disabled="busy"
              @input="handleInput"
            ></textarea>
          </section>

          <section v-else class="control-section operation-fields">
            <div class="section-title">定位与插入</div>
            <label class="field-label" for="afterInput">定位文本</label>
            <textarea
              id="afterInput"
              v-model="after"
              class="text-input text-area"
              rows="3"
              placeholder="在每次出现的位置之后插入"
              :disabled="busy"
              @input="handleInput"
            ></textarea>
            <label class="field-label" for="insertInput">插入内容</label>
            <textarea
              id="insertInput"
              v-model="insertText"
              class="text-input text-area"
              rows="3"
              placeholder="输入要插入的内容"
              :disabled="busy"
              @input="handleInput"
            ></textarea>
          </section>

          <div class="actions">
            <button class="button button-primary" type="button" :disabled="busy || !canPreview" @click="runPreview">预览更改</button>
            <div class="secondary-actions">
              <button class="button button-success" type="button" :disabled="busy || !preview || !canPreview" @click="applyChanges">应用更改</button>
              <button class="button button-secondary" type="button" :disabled="busy || !preview" @click="clearPreview">清除预览</button>
            </div>
          </div>

          <section v-if="preview" class="control-section">
            <div class="section-title">扫描结果</div>
            <div class="summary-card surface-card">
              <div class="summary-row"><span><i class="summary-dot dot-neutral"></i>扫描文件</span><strong>{{ preview.filesScanned }}</strong></div>
              <div class="summary-row"><span><i class="summary-dot dot-blue"></i>将修改</span><strong>{{ preview.filesModified }}</strong></div>
              <div class="summary-row"><span><i class="summary-dot dot-green"></i>修改处数</span><strong>{{ preview.totalEdits }}</strong></div>
              <div v-if="preview.binarySkipped > 0" class="summary-row"><span><i class="summary-dot dot-orange"></i>跳过二进制</span><strong>{{ preview.binarySkipped }}</strong></div>
            </div>
          </section>
        </div>
      </aside>

      <section class="preview-pane">
        <div class="preview-heading">
          <div>
            <div class="title-line">
              <h2>预览更改</h2>
              <span v-if="preview" class="preview-state" :class="preview.filesModified === 0 ? 'muted' : 'blue'">
                {{ preview.filesModified === 0 ? '没有发现改动' : '实时差异' }}
              </span>
            </div>
            <p>确认写入文件之前，先检查每一处变化</p>
          </div>
        </div>

        <div class="preview-body">
          <template v-if="preview && preview.filesModified > 0">
            <div class="metrics-grid">
              <div class="metric-card surface-card"><span>将修改文件</span><strong class="blue-text">{{ preview.filesModified }}</strong></div>
              <div class="metric-card surface-card"><span>修改处数</span><strong class="green-text">{{ preview.totalEdits }}</strong></div>
              <div class="metric-card surface-card"><span>扫描文件</span><strong class="neutral-text">{{ preview.filesScanned }}</strong></div>
            </div>

            <div class="diff-card surface-card">
              <div class="diff-heading"><strong>变更详情</strong><span>逐行对比</span></div>
              <div class="diff-scroll">
                <section v-for="file in preview.details" :key="file.path" class="file-block">
                  <div class="file-header">
                    <span class="file-badge">文件</span>
                    <span class="file-path">{{ file.path }}</span>
                    <span class="file-count">{{ file.diffs.length }} 处修改</span>
                  </div>
                  <div v-for="diff in file.diffs" :key="`${file.path}-${diff.lineNo}-${diff.old}`" class="diff-row">
                    <span class="line-number">{{ String(diff.lineNo).padStart(3, ' ') }}</span>
                    <div class="change-lines">
                      <div class="old-line">- {{ diff.old }}</div>
                      <div class="new-line">+ {{ diff.new }}</div>
                    </div>
                  </div>
                </section>
              </div>
            </div>
          </template>

          <div v-else class="empty-state surface-card">
            <div class="empty-symbol">✦</div>
            <h3>{{ emptyState.title }}</h3>
            <p>{{ emptyState.body }}</p>
            <span>{{ emptyState.note }}</span>
          </div>
        </div>
      </section>
    </main>
  </div>
</template>
