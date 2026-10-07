// SIM-015/SIM-028: srandom, get_randstate and set_randstate through a handle
// act on the named process's stream, not the caller's (SV 18.13-18.14).
module tb;
  process worker, twin;
  string saved, mine;
  int first[3], second[3], third[3];
  event go1, go2, go3;

  function automatic bit same(int a[3], int b[3]);
    foreach (a[i]) if (a[i] != b[i]) return 0;
    return 1;
  endfunction

  initial begin
    fork
      begin
        worker = process::self();
        @go1;
        foreach (first[i]) first[i] = $urandom;
        @go2;
        foreach (second[i]) second[i] = $urandom;
      end
      begin
        twin = process::self();
        @go3;
        foreach (third[i]) third[i] = $urandom;
      end
    join_none
    #1;
    mine = process::self().get_randstate();
    worker.srandom(1234);
    twin.srandom(1234);
    saved = worker.get_randstate();
    $display("caller stream unchanged %0d", mine == process::self().get_randstate());
    ->go1;
    #1;
    worker.set_randstate(saved);
    ->go2;
    ->go3;
    #1;
    $display("restored %0d", same(first, second));
    $display("same seed %0d", same(first, third));
    $finish;
  end
endmodule
