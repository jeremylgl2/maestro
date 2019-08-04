#include <process/process.h>

int write(int fildes, const void *buf, size_t nbyte);
pid_t fork(void);
void _exit(int status);
pid_t waitpid(pid_t pid, int *wstatus, int options);

/*static void putchar(char c)
{
	write(0, &c, 1);
}

static void putnbr(int n)
{
	if(n < 0)
	{
		putchar('-');
		n = -n;
	}
	if(n > 9)
		putnbr(n / 10);
	putchar('0' + (n % 10));
}*/

/*static void fork_bomb(void)
{
	pid_t pid;
	int status;

	if((pid = fork()) < 0)
	{
		write(0, "END\n", 4);
		_exit(1);
	}
	if(pid == 0)
		fork_bomb();
	else
	{
		waitpid(pid, &status, 0);
		_exit(status); // TODO EXITSTATUS
	}
}*/

void test_process(void)
{
	// TODO Single step analysis (infinite loop)
	/*ssize_t i;
	for(size_t j = 0; j < 1000; ++j)
		i = write(0, "Hello world!\r", 13);*/
	// TODO Writes `-0`?
	/*i = write(0, NULL, 13);
	putnbr(i);*/
	// TODO fork_bomb();
	write(0, "Hello world!\n", 13);
	while(1)
		;
	// TODO Protect when returning (Triple fault)
}