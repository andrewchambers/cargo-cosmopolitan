/* Compare the upstream C runtime with the Rust demo when testing portability. */
#include <pthread.h>
#include <stdio.h>

static void *worker(void *arg) {
  puts("C worker entered");
  pthread_mutex_t mutex;
  pthread_mutexattr_t attr;
  if (pthread_mutexattr_init(&attr)) return (void *)1;
  if (pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_ERRORCHECK)) return (void *)1;
  if (pthread_mutex_init(&mutex, &attr)) return (void *)1;
  pthread_mutexattr_destroy(&attr);
  if (pthread_mutex_lock(&mutex)) return (void *)1;
  if (pthread_mutex_unlock(&mutex)) return (void *)1;
  if (pthread_mutex_destroy(&mutex)) return (void *)1;
  puts("C mutex checked");
  return arg;
}

int main(void) {
  pthread_t thread;
  void *result;
  puts("C pthread_create");
  fflush(stdout);
  int error = pthread_create(&thread, 0, worker, 0);
  if (error) return error;
  error = pthread_join(thread, &result);
  puts("C pthread_join returned");
  return error || result != 0;
}
