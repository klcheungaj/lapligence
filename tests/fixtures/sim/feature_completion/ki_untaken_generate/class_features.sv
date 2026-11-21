module tb;
  class Base;
    virtual function int f();
      return 1;
    endfunction
  endclass
  if (0) begin : x
    class E;
      rand int a;
      constraint c { a inside {[1:5]}; }
      event ev;
      static int cnt;
      int q[$];
      mailbox #(E) mb;
      process p;
      function new();
        mb = new;
      endfunction
      virtual task run();
        fork
          begin
            p = process::self();
            #1 cnt++;
          end
        join_none
        -> ev;
      endtask
      static function int count();
        static int calls;
        calls++;
        return calls;
      endfunction
    endclass
    class D extends Base;
      function int f();
        return 2;
      endfunction
    endclass
    class P #(int N = 1);
      static int v = N;
    endclass
    virtual class IC;
      pure virtual function int g();
    endclass
    class Impl extends IC;
      virtual function int g();
        return 3;
      endfunction
    endclass
    int aa[E];
    E pool[$];
    initial begin
      E e;
      D d;
      Base b;
      Impl i;
      e = new;
      void'(e.randomize());
      void'(e.randomize() with { a == 2; });
      e.run();
      @(e.ev);
      d = new;
      b = d;
      void'($cast(d, b));
      i = new;
      $display(i.g(), P#(4)::v, E::count());
      aa[e] = 1;
      pool.push_back(e);
      e.mb.put(e);
      $display("untaken ran");
    end
  end
  initial begin
    #2 $display("done");
    $finish;
  end
endmodule
