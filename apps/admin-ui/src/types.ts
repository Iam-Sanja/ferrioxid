export type Field = {
  name: string;
  label: string;
  type: "string" | "integer" | "boolean" | "uuid";
  required: boolean;
  readonly: boolean;
  nullable: boolean;
  searchable: boolean;
  sortable: boolean;
  filterable: boolean;
  list_display: boolean;
  foreign_key: string | null;
};

export type Schema = {
  model: string;
  label: string;
  list_blocked: boolean;
  actions: string[];
  fields: Field[];
};

export type ModelLink = {
  key: string;
  label: string;
  list_blocked: boolean;
};

export type PublicUser = {
  id: string;
  email: string;
  is_staff: boolean;
  is_superuser: boolean;
};

export type ApiError = {
  status: number;
  detail: string;
};
