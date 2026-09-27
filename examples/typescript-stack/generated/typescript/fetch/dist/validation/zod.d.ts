import { z } from 'zod';
export declare const PetSchema: z.ZodObject<{
    id: z.ZodString;
    name: z.ZodString;
    species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
}, "strip", z.ZodUnknown, z.objectOutputType<{
    id: z.ZodString;
    name: z.ZodString;
    species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
}, z.ZodUnknown, "strip">, z.objectInputType<{
    id: z.ZodString;
    name: z.ZodString;
    species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
}, z.ZodUnknown, "strip">>;
export type Pet = z.infer<typeof PetSchema>;
export declare const PetListSchema: z.ZodObject<{
    data: z.ZodArray<z.ZodLazy<z.ZodObject<{
        id: z.ZodString;
        name: z.ZodString;
        species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
    }, "strip", z.ZodUnknown, z.objectOutputType<{
        id: z.ZodString;
        name: z.ZodString;
        species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
    }, z.ZodUnknown, "strip">, z.objectInputType<{
        id: z.ZodString;
        name: z.ZodString;
        species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
    }, z.ZodUnknown, "strip">>>, "many">;
}, "strip", z.ZodUnknown, z.objectOutputType<{
    data: z.ZodArray<z.ZodLazy<z.ZodObject<{
        id: z.ZodString;
        name: z.ZodString;
        species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
    }, "strip", z.ZodUnknown, z.objectOutputType<{
        id: z.ZodString;
        name: z.ZodString;
        species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
    }, z.ZodUnknown, "strip">, z.objectInputType<{
        id: z.ZodString;
        name: z.ZodString;
        species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
    }, z.ZodUnknown, "strip">>>, "many">;
}, z.ZodUnknown, "strip">, z.objectInputType<{
    data: z.ZodArray<z.ZodLazy<z.ZodObject<{
        id: z.ZodString;
        name: z.ZodString;
        species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
    }, "strip", z.ZodUnknown, z.objectOutputType<{
        id: z.ZodString;
        name: z.ZodString;
        species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
    }, z.ZodUnknown, "strip">, z.objectInputType<{
        id: z.ZodString;
        name: z.ZodString;
        species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
    }, z.ZodUnknown, "strip">>>, "many">;
}, z.ZodUnknown, "strip">>;
export type PetList = z.infer<typeof PetListSchema>;
export declare const CreatePetSchema: z.ZodObject<{
    name: z.ZodString;
    species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
}, "strip", z.ZodUnknown, z.objectOutputType<{
    name: z.ZodString;
    species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
}, z.ZodUnknown, "strip">, z.objectInputType<{
    name: z.ZodString;
    species: z.ZodUnion<[z.ZodLiteral<"cat">, z.ZodLiteral<"dog">, z.ZodLiteral<"rabbit">]>;
}, z.ZodUnknown, "strip">>;
export type CreatePet = z.infer<typeof CreatePetSchema>;
export declare const ErrorSchema: z.ZodObject<{
    message: z.ZodString;
}, "strip", z.ZodUnknown, z.objectOutputType<{
    message: z.ZodString;
}, z.ZodUnknown, "strip">, z.objectInputType<{
    message: z.ZodString;
}, z.ZodUnknown, "strip">>;
export type Error = z.infer<typeof ErrorSchema>;
