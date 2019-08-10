#include <memory/kmalloc/kmalloc.h>
#include <libc/errno.h>

spinlock_t kmalloc_spinlock = 0;

__attribute__((section("bss")))
static chunk_t *buckets[BUCKETS_COUNT];
__attribute__((section("bss")))
static chunk_t *large_chunks;

chunk_t *get_chunk(void *ptr)
{
	if(!ptr)
		return NULL;
	// TODO
	return NULL;
}

static void coalesce_chunks(chunk_t *chunk)
{
	chunk_t *begin, *end;
	size_t size = 0;

	if(chunk->used)
		return;
	begin = chunk;
	end = chunk;
	while(begin->prev && !begin->prev->used && SAME_PAGE(begin, begin->prev))
		begin = begin->prev;
	while(end && !end->used && SAME_PAGE(end, end->prev))
		end = end->next;
	if(begin == end)
		return;
	chunk = begin;
	while(chunk != end)
	{
		size += chunk->size;
		if(chunk->next != end)
			size += sizeof(chunk_t);
		chunk = chunk->next;
	}
	begin->size = size;
	begin->next = end;
}

static void *_alloc_block(const size_t pages, const int flags)
{
	if(flags & KMALLOC_BUDDY)
		return buddy_alloc(buddy_get_order(pages * PAGE_SIZE));
	else
		return pages_alloc(pages);
}

static void alloc_block(chunk_t **bucket, const int flags)
{
	chunk_t *ptr;

	if(!(ptr = _alloc_block(1, flags)))
		return;
	ptr->next = *bucket;
	ptr->size = PAGE_SIZE - sizeof(chunk_t);
	*bucket = ptr;
	coalesce_chunks(ptr);
}

static void *large_alloc(const size_t size, const int flags)
{
	size_t total_size, pages;
	chunk_t *chunk;

	total_size = sizeof(chunk_t) + size;
	pages = UPPER_DIVISION(total_size, PAGE_SIZE);
	if((chunk = _alloc_block(pages, flags)))
	{
		chunk->size = pages * PAGE_SIZE + sizeof(chunk_t);
		chunk->next = large_chunks;
		large_chunks = chunk;
	}
	return chunk;
}

static chunk_t *bucket_get_free_chunk(chunk_t **bucket, const size_t size)
{
	chunk_t *c;

	c = *bucket;
	while(c && (c->used || c->size < size))
		c = c->next;
	return c;
}

chunk_t *get_free_chunk(const size_t size, const int flags)
{
	size_t i = 0;
	chunk_t **bucket, *c;

	while(SMALLER_BUCKET * POW2(i) < size)
		++i;
	if(i < BUCKETS_COUNT)
		bucket = buckets + i;
	else
		return large_alloc(size, flags);
	if(!(c = bucket_get_free_chunk(bucket, size)))
	{
		alloc_block(bucket, flags);
		if(!errno)
			return NULL;
		c = bucket_get_free_chunk(bucket, size);
	}
	return c;
}

void alloc_chunk(chunk_t *chunk, const size_t size)
{
	chunk_t *next;

	if(!chunk)
		return;
	if(chunk->size + sizeof(chunk_t) > size)
	{
		next = (void *) chunk + size;
		next->prev = chunk;
		next->next = chunk->next;
		next->size = chunk->size - size - sizeof(chunk_t);
		next->used = 0;
		chunk->next = next;
	}
	chunk->size = size;
	chunk->used = 1;
}

void free_chunk(chunk_t *chunk, const int flags)
{
	if(!chunk)
		return;
	if(chunk->prev)
		chunk->prev->next = chunk->next;
	if(chunk->next)
		chunk->next->prev = chunk->prev;
	chunk->used = 0;
	coalesce_chunks(chunk);
	// TODO If page is empty, free it
	(void) flags;
}
