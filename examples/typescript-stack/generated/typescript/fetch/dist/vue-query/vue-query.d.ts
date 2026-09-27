import { listPets } from "../clients/pets/listPets";
export declare const listPetsQueryKey: (options: Parameters<typeof listPets>[0]) => readonly ["listPets", import("../.kaji/client").Options<import("..").ListPetsOptions, boolean>];
export declare function useListPets(options: Parameters<typeof listPets>[0]): import("@tanstack/vue-query").UseQueryDefinedReturnType<unknown, Error>;
export declare function useCreatePet(): any;
import { getPet } from "../clients/pets/getPet";
export declare const getPetQueryKey: (options: Parameters<typeof getPet>[0]) => readonly ["getPet", import("../.kaji/client").Options<import("..").GetPetOptions, boolean>];
export declare function useGetPet(options: Parameters<typeof getPet>[0]): import("@tanstack/vue-query").UseQueryDefinedReturnType<unknown, Error>;
