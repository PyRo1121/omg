import { APIEvent } from "@solidjs/start/server";
import { sql, eq, desc, and } from "drizzle-orm";
import * as schema from "~/db/auth-schema";
import { requireAdmin } from "~/lib/admin";

export async function GET(event: APIEvent) {
  try {
    const adminCheck = await requireAdmin(event);
    if (adminCheck instanceof Response) return adminCheck;

    const { db, userId: adminId } = adminCheck;

    const url = new URL(event.request.url);
    const customerId = url.searchParams.get("customerId");
    const limit = Math.min(parseInt(url.searchParams.get("limit") || "50"), 100);
    const offset = parseInt(url.searchParams.get("offset") || "0");
    const noteType = url.searchParams.get("type");

    if (!customerId) {
      return new Response(
        JSON.stringify({ error: "customerId is required" }),
        {
          status: 400,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Verify customer exists
    const customer = await db
      .select({ id: schema.user.id })
      .from(schema.user)
      .where(eq(schema.user.id, customerId))
      .limit(1)
      .get();

    if (!customer) {
      return new Response(
        JSON.stringify({ error: "Customer not found" }),
        {
          status: 404,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Build query conditions
    const conditions = [eq(schema.customerNote.userId, customerId)];
    if (noteType) {
      conditions.push(eq(schema.customerNote.noteType, noteType as any));
    }

    // Get notes with author info
    const notes = await db
      .select({
        id: schema.customerNote.id,
        content: schema.customerNote.content,
        noteType: schema.customerNote.noteType,
        isPinned: schema.customerNote.isPinned,
        createdAt: schema.customerNote.createdAt,
        updatedAt: schema.customerNote.updatedAt,
        authorId: schema.customerNote.authorId,
        authorName: schema.user.name,
        authorEmail: schema.user.email,
      })
      .from(schema.customerNote)
      .innerJoin(schema.user, eq(schema.customerNote.authorId, schema.user.id))
      .where(and(...conditions))
      .orderBy(
        desc(schema.customerNote.isPinned),
        desc(schema.customerNote.createdAt)
      )
      .limit(limit)
      .offset(offset)
      .all();

    // Get total count
    const totalCount = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.customerNote)
      .where(and(...conditions))
      .get();

    return new Response(
      JSON.stringify({
        notes: notes.map(n => ({
          id: n.id,
          content: n.content,
          noteType: n.noteType,
          isPinned: n.isPinned,
          createdAt: new Date(n.createdAt).toISOString(),
          updatedAt: new Date(n.updatedAt).toISOString(),
          author: {
            id: n.authorId,
            name: n.authorName,
            email: n.authorEmail,
          },
        })),
        pagination: {
          limit,
          offset,
          total: Number(totalCount?.count || 0),
        },
      }),
      {
        status: 200,
        headers: {
          "Content-Type": "application/json",
          "Cache-Control": "private, no-cache, no-store, must-revalidate",
        },
      }
    );
  } catch (error) {
    console.error("[Admin CRM Notes GET] Error:", error);
    return new Response(
      JSON.stringify({
        error: "Internal server error",
        message: error instanceof Error ? error.message : "Unknown error",
      }),
      {
        status: 500,
        headers: { "Content-Type": "application/json" },
      }
    );
  }
}

export async function POST(event: APIEvent) {
  try {
    const adminCheck = await requireAdmin(event);
    if (adminCheck instanceof Response) return adminCheck;

    const { db, userId: adminId } = adminCheck;

    const body = await event.request.json();
    const { customerId, content, noteType, isPinned } = body;

    if (!customerId || !content) {
      return new Response(
        JSON.stringify({ error: "customerId and content are required" }),
        {
          status: 400,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Verify customer exists
    const customer = await db
      .select({ id: schema.user.id })
      .from(schema.user)
      .where(eq(schema.user.id, customerId))
      .limit(1)
      .get();

    if (!customer) {
      return new Response(
        JSON.stringify({ error: "Customer not found" }),
        {
          status: 404,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Validate note type
    const validNoteTypes = ["general", "call", "meeting", "support", "escalation"];
    const type = noteType && validNoteTypes.includes(noteType) ? noteType : "general";

    const noteId = crypto.randomUUID();
    const now = new Date();

    await db
      .insert(schema.customerNote)
      .values({
        id: noteId,
        userId: customerId,
        authorId: adminId,
        content: content.trim(),
        noteType: type,
        isPinned: isPinned || false,
        createdAt: now,
        updatedAt: now,
      })
      .run();

    // Fetch the created note with author info
    const createdNote = await db
      .select({
        id: schema.customerNote.id,
        content: schema.customerNote.content,
        noteType: schema.customerNote.noteType,
        isPinned: schema.customerNote.isPinned,
        createdAt: schema.customerNote.createdAt,
        updatedAt: schema.customerNote.updatedAt,
        authorId: schema.customerNote.authorId,
        authorName: schema.user.name,
        authorEmail: schema.user.email,
      })
      .from(schema.customerNote)
      .innerJoin(schema.user, eq(schema.customerNote.authorId, schema.user.id))
      .where(eq(schema.customerNote.id, noteId))
      .limit(1)
      .get();

    return new Response(
      JSON.stringify({
        success: true,
        note: {
          id: createdNote!.id,
          content: createdNote!.content,
          noteType: createdNote!.noteType,
          isPinned: createdNote!.isPinned,
          createdAt: new Date(createdNote!.createdAt).toISOString(),
          updatedAt: new Date(createdNote!.updatedAt).toISOString(),
          author: {
            id: createdNote!.authorId,
            name: createdNote!.authorName,
            email: createdNote!.authorEmail,
          },
        },
      }),
      {
        status: 201,
        headers: { "Content-Type": "application/json" },
      }
    );
  } catch (error) {
    console.error("[Admin CRM Notes POST] Error:", error);
    return new Response(
      JSON.stringify({
        error: "Internal server error",
        message: error instanceof Error ? error.message : "Unknown error",
      }),
      {
        status: 500,
        headers: { "Content-Type": "application/json" },
      }
    );
  }
}

export async function PUT(event: APIEvent) {
  try {
    const adminCheck = await requireAdmin(event);
    if (adminCheck instanceof Response) return adminCheck;

    const { db, userId: adminId } = adminCheck;

    const body = await event.request.json();
    const { noteId, content, noteType, isPinned } = body;

    if (!noteId) {
      return new Response(
        JSON.stringify({ error: "noteId is required" }),
        {
          status: 400,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Verify note exists
    const existingNote = await db
      .select()
      .from(schema.customerNote)
      .where(eq(schema.customerNote.id, noteId))
      .limit(1)
      .get();

    if (!existingNote) {
      return new Response(
        JSON.stringify({ error: "Note not found" }),
        {
          status: 404,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Build update object
    const updates: Record<string, any> = {
      updatedAt: new Date(),
    };

    if (content !== undefined) {
      updates.content = content.trim();
    }
    if (noteType !== undefined) {
      const validNoteTypes = ["general", "call", "meeting", "support", "escalation"];
      if (validNoteTypes.includes(noteType)) {
        updates.noteType = noteType;
      }
    }
    if (isPinned !== undefined) {
      updates.isPinned = isPinned;
    }

    await db
      .update(schema.customerNote)
      .set(updates)
      .where(eq(schema.customerNote.id, noteId))
      .run();

    return new Response(
      JSON.stringify({ success: true }),
      {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }
    );
  } catch (error) {
    console.error("[Admin CRM Notes PUT] Error:", error);
    return new Response(
      JSON.stringify({
        error: "Internal server error",
        message: error instanceof Error ? error.message : "Unknown error",
      }),
      {
        status: 500,
        headers: { "Content-Type": "application/json" },
      }
    );
  }
}

export async function DELETE(event: APIEvent) {
  try {
    const adminCheck = await requireAdmin(event);
    if (adminCheck instanceof Response) return adminCheck;

    const { db } = adminCheck;

    const url = new URL(event.request.url);
    const noteId = url.searchParams.get("noteId");

    if (!noteId) {
      return new Response(
        JSON.stringify({ error: "noteId is required" }),
        {
          status: 400,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Verify note exists
    const existingNote = await db
      .select({ id: schema.customerNote.id })
      .from(schema.customerNote)
      .where(eq(schema.customerNote.id, noteId))
      .limit(1)
      .get();

    if (!existingNote) {
      return new Response(
        JSON.stringify({ error: "Note not found" }),
        {
          status: 404,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    await db
      .delete(schema.customerNote)
      .where(eq(schema.customerNote.id, noteId))
      .run();

    return new Response(
      JSON.stringify({ success: true }),
      {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }
    );
  } catch (error) {
    console.error("[Admin CRM Notes DELETE] Error:", error);
    return new Response(
      JSON.stringify({
        error: "Internal server error",
        message: error instanceof Error ? error.message : "Unknown error",
      }),
      {
        status: 500,
        headers: { "Content-Type": "application/json" },
      }
    );
  }
}
