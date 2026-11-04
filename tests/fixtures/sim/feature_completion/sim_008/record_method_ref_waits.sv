// SV 13.5.2: methods, one per class of a virtual family, waiting on members of a module record `ref` actual.
module tb;
  typedef struct { string s; int n; } rec_t;
  class K;
    task w(ref rec_t x);
      @(x.n);
      $display("%0d K w n=%0d", $time, x.n);
    endtask
    virtual task v(ref rec_t x);
      wait (x.n == 5);
      $display("%0d K v n=%0d", $time, x.n);
    endtask
  endclass
  class D extends K;
    virtual task v(ref rec_t x);
      @(x.s);
      $display("%0d D v s=%s", $time, x.s);
    endtask
  endclass
  rec_t mr;
  K k;
  D d;
  K b;
  initial begin
    k = new; d = new; b = d;
    fork
      k.w(mr);
      k.v(mr);
      d.w(mr);
      b.v(mr);
      begin
        #1 mr.n = 3;
        #1 mr.s = "go";
        #1 mr.n = 5;
      end
    join
    $finish;
  end
endmodule
