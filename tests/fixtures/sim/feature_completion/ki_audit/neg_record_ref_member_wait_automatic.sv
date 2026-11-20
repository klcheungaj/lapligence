// An event control on a member of a `ref` formal whose actual is an
// automatic record of a subroutine: a located limit (docs/known_issues.md,
// unpacked records with dense member arrays).
module tb;
  typedef struct { int a[16]; string s; } rec_t;
  task automatic watch(ref rec_t r);
    @(r.a[0]);
    $display("changed %0d", r.a[0]);
  endtask
  task automatic run();
    rec_t local_r;
    fork
      watch(local_r);
      #1 local_r.a[0] = 4;
    join
  endtask
  initial begin
    run();
    $finish(0);
  end
endmodule
