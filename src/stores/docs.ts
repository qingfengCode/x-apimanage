import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import type { DocumentItem } from "@/types";

interface DocsState {
  documents: DocumentItem[];
  loaded: boolean;
}

export const useDocsStore = defineStore("docs", {
  state: (): DocsState => ({
    documents: [],
    loaded: false,
  }),

  actions: {
    async init() {
      this.documents = await invoke<DocumentItem[]>("list_documents");
      this.loaded = true;
    },

    /** 保存后刷新列表（本地直接替换，避免全量拉取） */
    async save(input: {
      id?: string;
      docType: string;
      title: string;
      content: string;
      requestId?: string | null;
    }): Promise<DocumentItem> {
      const saved = await invoke<DocumentItem>("save_document", { input });
      const idx = this.documents.findIndex((d) => d.id === saved.id);
      if (idx >= 0) this.documents.splice(idx, 1, saved);
      else this.documents.unshift(saved);
      return saved;
    },

    async remove(id: string) {
      await invoke("delete_document", { id });
      this.documents = this.documents.filter((d) => d.id !== id);
    },
  },
});
