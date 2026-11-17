// A task that waits on a record `ref` formal bound to an element of a
// subroutine record with a queue member takes the record's companion
// containers, whose changes publish nothing the wait could follow: the call
// rejects instead of waiting forever (SIM-009).
module tb;
  typedef struct { string s; int n; int q[$]; } el_t;
  typedef struct { el_t e[2]; } rec_t;
  task automatic waiter(ref el_t r);
    wait (r.q.size() == 1);
  endtask
  task automatic body();
    rec_t t;
    fork
      waiter(t.e[1]);
    join_none
    #1 t.e[1].q.push_back(5);
  endtask
  initial body();
endmodule
