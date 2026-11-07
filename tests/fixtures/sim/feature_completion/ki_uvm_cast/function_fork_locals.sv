class C;
  int a;
  function new(int x = 0);
    a = x;
  endfunction
endclass
class D extends C;
  function new(int x = 0);
    super.new(x);
  endfunction
  function void hi();
    $display("D %0d at %0d", a, $time);
  endfunction
endclass
class P;
  int tag = 1;
  function void cast_from_formal(C comp);
    fork
      begin
        D d;
        #1;
        if ($cast(d, comp)) d.hi();
        else $display("not D %0d", comp.a);
      end
    join_none
  endfunction
  function void two_activations(int v);
    fork
      begin
        C l = new(v);
        C n;
        n = l;
        #2;
        $display("%0d %0d", l.a, n.a);
      end
    join_none
  endfunction
  function void nested(int v);
    fork
      begin
        C outer_h = new(v);
        fork
          begin
            C inner_h;
            #1 inner_h = outer_h;
            $display("nested %0d", inner_h.a);
          end
        join_none
      end
    join_none
  endfunction
endclass
module tb;
  function automatic void in_module(int v);
    fork
      begin
        C l;
        l = new(v);
        #1 $display("module %0d", l.a);
      end
    join_none
  endfunction
  P p = new;
  D d = new(4);
  C c = new(5);
  initial begin
    p.cast_from_formal(d);
    p.cast_from_formal(c);
    p.two_activations(10);
    p.two_activations(11);
    p.nested(20);
    in_module(30);
    in_module(31);
    #5 $finish;
  end
endmodule
