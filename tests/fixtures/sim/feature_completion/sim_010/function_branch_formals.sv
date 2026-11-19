// SIM-010: a function's fork-join_none branches use its string formals and
// locals and its record formals after the function returned (SV 6.21, 9.3.2,
// 13.4).
module tb;
  typedef struct {
    string name;
    int    q[$];
  } rec_t;

  task automatic shs(input string s);
    $display("shs %s", s);
  endtask
  task automatic app(inout string s);
    s = {s, "+app"};
  endtask

  class K;
    function void f(input string s, input int n);
      fork
        begin
          #1 $display("k.f %s %0d", s, n);
        end
      join_none
    endfunction
  endclass

  function automatic void g(input string s);
    string l;
    l = {s, "-l"};
    fork
      begin
        #2 s = {s, "+w"};
        app(l);
        shs(s);
        $display("g %s %s %0d", s, l, s.len());
      end
    join_none
    s = "after";
  endfunction

  function automatic void r(input rec_t x, input string tag);
    fork
      begin
        #3 x.q.push_back(4);
        fork
          begin
            #1 $display("r nested %s %s %0d", tag, x.name, x.q.size());
          end
        join_none
        $display("r %s %0d", x.name, x.q[$]);
      end
    join_none
    x.name = {x.name, "!"};
  endfunction

  initial begin
    K k;
    rec_t v;
    k = new;
    k.f("kf", 3);
    g("gg");
    v.name = "v";
    v.q.push_back(1);
    r(v, "t");
    #5 $display("end %s %0d", v.name, v.q.size());
    $finish;
  end
endmodule
