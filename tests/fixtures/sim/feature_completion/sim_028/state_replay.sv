// SIM-028 A02: saving and restoring a thread's random state replays its next
// values, for the thread itself and through a process handle (IEEE 1800-2009
// 9.7, 18.13.1-18.13.5, 18.14.2). The 2-state `int` arguments of srandom,
// $urandom and $urandom_range read X/Z bits as 0 (6.11.2) and round reals
// (6.12.2). Only relations are checked.
module tb;
  int unsigned first[6], second[6];
  int q[$];
  string saved, worker_saved;
  int unsigned worker_values[4];
  logic [31:0] unknown;
  int unsigned x, y;
  process worker;
  event go, resume_worker;
  bit mismatch;

  int unsigned values[6];

  task automatic draw();
    values[0] = $urandom;
    values[1] = $urandom_range(100, 10);
    q = '{1, 2, 3, 4, 5};
    q.shuffle();
    values[2] = q[0] * 10000 + q[1] * 1000 + q[2] * 100 + q[3] * 10 + q[4];
    fork
      values[3] = $urandom;
    join
    values[4] = $urandom;
    values[5] = $urandom_range(7);
  endtask

  initial begin : worker_thread
    worker = process::self();
    @(go);
    for (int i = 0; i < 2; i++) worker_values[i] = $urandom;
    @(resume_worker);
    for (int i = 2; i < 4; i++) worker_values[i] = $urandom;
  end

  initial begin : main
    process::self().srandom(21);
    void'($urandom);
    saved = process::self().get_randstate();
    draw();
    first = values;
    process::self().set_randstate(saved);
    draw();
    second = values;
    mismatch = 0;
    foreach (first[i]) if (first[i] != second[i]) mismatch = 1;
    $display("self replay %0d", !mismatch);

    // Another process's stream, saved and restored through its handle.
    #1;
    worker_saved = worker.get_randstate();
    -> go;
    #1;
    worker.set_randstate(worker_saved);
    -> resume_worker;
    #1;
    $display("handle replay %0d %0d", worker_values[0] == worker_values[2],
             worker_values[1] == worker_values[3]);

    unknown = 'x;
    process::self().srandom(unknown);
    x = $urandom;
    process::self().srandom(0);
    y = $urandom;
    $display("unknown srandom seed is zero %0d", x == y);
    x = $urandom(unknown);
    y = $urandom(0);
    $display("unknown urandom seed is zero %0d", x == y);
    process::self().srandom(5);
    x = $urandom_range(unknown, 3);
    process::self().srandom(5);
    y = $urandom_range(0, 3);
    $display("unknown range bound is zero %0d %0d", x == y, x <= 3);
    // Real arguments convert to the 2-state formals by rounding.
    process::self().srandom(2.6);
    x = $urandom;
    process::self().srandom(3);
    y = $urandom;
    $display("real srandom seed rounds %0d", x == y);
    x = $urandom(2.6);
    y = $urandom(3);
    $display("real urandom seed rounds %0d", x == y);
    process::self().srandom(5);
    x = $urandom_range(6.6, 1.4);
    process::self().srandom(5);
    y = $urandom_range(7, 1);
    $display("real range bounds round %0d", x == y);
    $finish;
  end
endmodule
