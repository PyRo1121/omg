import { APIEvent } from "@solidjs/start/server";
import { sql, eq, desc, and, inArray } from "drizzle-orm";
import * as schema from "~/db/auth-schema";
import { requireAdmin } from "~/lib/admin";

export async function GET(event: APIEvent) {
  try {
    const adminCheck = await requireAdmin(event);
    if (adminCheck instanceof Response) return adminCheck;

    const { db } = adminCheck;

    const url = new URL(event.request.url);
    const customerId = url.searchParams.get("customerId");
    const limit = Math.min(parseInt(url.searchParams.get("limit") || "100"), 200);
    const offset = parseInt(url.searchParams.get("offset") || "0");

    if (customerId) {
      // Get tags for a specific customer
      const customerTags = await db
        .select({
          id: schema.customerTag.id,
          name: schema.customerTag.name,
          color: schema.customerTag.color,
          description: schema.customerTag.description,
          assignedAt: schema.customerTagAssignment.createdAt,
          assignedById: schema.customerTagAssignment.assignedBy,
        })
        .from(schema.customerTagAssignment)
        .innerJoin(
          schema.customerTag,
          eq(schema.customerTagAssignment.tagId, schema.customerTag.id)
        )
        .where(eq(schema.customerTagAssignment.userId, customerId))
        .orderBy(desc(schema.customerTagAssignment.createdAt))
        .limit(limit)
        .offset(offset)
        .all();

      return new Response(
        JSON.stringify({
          tags: customerTags.map(t => ({
            id: t.id,
            name: t.name,
            color: t.color,
            description: t.description,
            assignedAt: new Date(t.assignedAt).toISOString(),
            assignedById: t.assignedById,
          })),
        }),
        {
          status: 200,
          headers: {
            "Content-Type": "application/json",
            "Cache-Control": "private, no-cache, no-store, must-revalidate",
          },
        }
      );
    }

    // Get all available tags with usage count
    const allTags = await db
      .select({
        id: schema.customerTag.id,
        name: schema.customerTag.name,
        color: schema.customerTag.color,
        description: schema.customerTag.description,
        createdAt: schema.customerTag.createdAt,
        usageCount: sql<number>`(
          SELECT COUNT(*)
          FROM customer_tag_assignment
          WHERE customer_tag_assignment.tag_id = ${schema.customerTag.id}
        )`,
      })
      .from(schema.customerTag)
      .orderBy(schema.customerTag.name)
      .limit(limit)
      .offset(offset)
      .all();

    const totalCount = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.customerTag)
      .get();

    return new Response(
      JSON.stringify({
        tags: allTags.map(t => ({
          id: t.id,
          name: t.name,
          color: t.color,
          description: t.description,
          createdAt: new Date(t.createdAt).toISOString(),
          usageCount: Number(t.usageCount),
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
    console.error("[Admin CRM Tags GET] Error:", error);
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
    const { customerId, tagId, name, color, description } = body;

    // Case 1: Assign existing tag to customer
    if (customerId && tagId) {
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

      // Verify tag exists
      const tag = await db
        .select({ id: schema.customerTag.id })
        .from(schema.customerTag)
        .where(eq(schema.customerTag.id, tagId))
        .limit(1)
        .get();

      if (!tag) {
        return new Response(
          JSON.stringify({ error: "Tag not found" }),
          {
            status: 404,
            headers: { "Content-Type": "application/json" },
          }
        );
      }

      // Check if assignment already exists
      const existingAssignment = await db
        .select({ id: schema.customerTagAssignment.id })
        .from(schema.customerTagAssignment)
        .where(
          and(
            eq(schema.customerTagAssignment.userId, customerId),
            eq(schema.customerTagAssignment.tagId, tagId)
          )
        )
        .limit(1)
        .get();

      if (existingAssignment) {
        return new Response(
          JSON.stringify({ error: "Tag already assigned to customer" }),
          {
            status: 409,
            headers: { "Content-Type": "application/json" },
          }
        );
      }

      const assignmentId = crypto.randomUUID();
      await db
        .insert(schema.customerTagAssignment)
        .values({
          id: assignmentId,
          userId: customerId,
          tagId: tagId,
          assignedBy: adminId,
          createdAt: new Date(),
        })
        .run();

      return new Response(
        JSON.stringify({
          success: true,
          assignmentId,
        }),
        {
          status: 201,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Case 2: Create a new tag
    if (name) {
      // Check if tag name already exists
      const existingTag = await db
        .select({ id: schema.customerTag.id })
        .from(schema.customerTag)
        .where(eq(schema.customerTag.name, name.trim()))
        .limit(1)
        .get();

      if (existingTag) {
        return new Response(
          JSON.stringify({ error: "Tag with this name already exists" }),
          {
            status: 409,
            headers: { "Content-Type": "application/json" },
          }
        );
      }

      const tagId = crypto.randomUUID();
      await db
        .insert(schema.customerTag)
        .values({
          id: tagId,
          name: name.trim(),
          color: color || "#6366f1",
          description: description?.trim() || null,
          createdAt: new Date(),
        })
        .run();

      // If customerId is also provided, assign the new tag
      if (customerId) {
        const customer = await db
          .select({ id: schema.user.id })
          .from(schema.user)
          .where(eq(schema.user.id, customerId))
          .limit(1)
          .get();

        if (customer) {
          await db
            .insert(schema.customerTagAssignment)
            .values({
              id: crypto.randomUUID(),
              userId: customerId,
              tagId: tagId,
              assignedBy: adminId,
              createdAt: new Date(),
            })
            .run();
        }
      }

      return new Response(
        JSON.stringify({
          success: true,
          tag: {
            id: tagId,
            name: name.trim(),
            color: color || "#6366f1",
            description: description?.trim() || null,
          },
        }),
        {
          status: 201,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    return new Response(
      JSON.stringify({ error: "Either (customerId and tagId) or name is required" }),
      {
        status: 400,
        headers: { "Content-Type": "application/json" },
      }
    );
  } catch (error) {
    console.error("[Admin CRM Tags POST] Error:", error);
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

    const { db } = adminCheck;

    const body = await event.request.json();
    const { tagId, name, color, description } = body;

    if (!tagId) {
      return new Response(
        JSON.stringify({ error: "tagId is required" }),
        {
          status: 400,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Verify tag exists
    const existingTag = await db
      .select()
      .from(schema.customerTag)
      .where(eq(schema.customerTag.id, tagId))
      .limit(1)
      .get();

    if (!existingTag) {
      return new Response(
        JSON.stringify({ error: "Tag not found" }),
        {
          status: 404,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Check for name conflict if updating name
    if (name && name.trim() !== existingTag.name) {
      const nameConflict = await db
        .select({ id: schema.customerTag.id })
        .from(schema.customerTag)
        .where(eq(schema.customerTag.name, name.trim()))
        .limit(1)
        .get();

      if (nameConflict) {
        return new Response(
          JSON.stringify({ error: "Tag with this name already exists" }),
          {
            status: 409,
            headers: { "Content-Type": "application/json" },
          }
        );
      }
    }

    // Build update object
    const updates: Record<string, any> = {};
    if (name !== undefined) updates.name = name.trim();
    if (color !== undefined) updates.color = color;
    if (description !== undefined) updates.description = description?.trim() || null;

    if (Object.keys(updates).length > 0) {
      await db
        .update(schema.customerTag)
        .set(updates)
        .where(eq(schema.customerTag.id, tagId))
        .run();
    }

    return new Response(
      JSON.stringify({ success: true }),
      {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }
    );
  } catch (error) {
    console.error("[Admin CRM Tags PUT] Error:", error);
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
    const tagId = url.searchParams.get("tagId");
    const customerId = url.searchParams.get("customerId");

    if (!tagId) {
      return new Response(
        JSON.stringify({ error: "tagId is required" }),
        {
          status: 400,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // If customerId is provided, remove tag assignment only
    if (customerId) {
      await db
        .delete(schema.customerTagAssignment)
        .where(
          and(
            eq(schema.customerTagAssignment.userId, customerId),
            eq(schema.customerTagAssignment.tagId, tagId)
          )
        )
        .run();

      return new Response(
        JSON.stringify({ success: true }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Otherwise, delete the tag entirely (cascade will remove assignments)
    const existingTag = await db
      .select({ id: schema.customerTag.id })
      .from(schema.customerTag)
      .where(eq(schema.customerTag.id, tagId))
      .limit(1)
      .get();

    if (!existingTag) {
      return new Response(
        JSON.stringify({ error: "Tag not found" }),
        {
          status: 404,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    await db
      .delete(schema.customerTag)
      .where(eq(schema.customerTag.id, tagId))
      .run();

    return new Response(
      JSON.stringify({ success: true }),
      {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }
    );
  } catch (error) {
    console.error("[Admin CRM Tags DELETE] Error:", error);
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
