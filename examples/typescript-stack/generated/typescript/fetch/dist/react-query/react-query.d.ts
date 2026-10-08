import { listPets } from "../clients/pets/listPets";
export declare const listPetsQueryKey: (options: Parameters<typeof listPets>[0]) => readonly ["listPets", import("../.poolster/client").Options<import("..").ListPetsOptions, boolean>];
export declare function useListPets(options: Parameters<typeof listPets>[0]): import("@tanstack/react-query").UseQueryResult<import("..").PetList, Error>;
export declare function useCreatePet(): import("@tanstack/react-query").UseMutationResult<import("..").Pet, Error, import("../.poolster/client").Options<import("..").CreatePetOptions, boolean>, unknown>;
import { getPet } from "../clients/pets/getPet";
export declare const getPetQueryKey: (options: Parameters<typeof getPet>[0]) => readonly ["getPet", import("../.poolster/client").Options<import("..").GetPetOptions, boolean>];
export declare function useGetPet(options: Parameters<typeof getPet>[0]): import("@tanstack/react-query").UseQueryResult<import("..").Error | import("..").Pet, Error>;
