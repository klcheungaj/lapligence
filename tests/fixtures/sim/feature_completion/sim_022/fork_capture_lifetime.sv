// SIM-022-A01: detached fork children keep the enclosing call's automatic
// native storage alive after the call returns. Each activation has its own
// storage, and a child sees the parent's writes made before the child reads.
module tb;
  class Box;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass
  typedef struct { string s; int q[$]; } rec_t;

  task automatic spawn(int k);
    string s = $sformatf("s%0d", k);
    int q[$] = '{k, k};
    Box b = new(k);
    rec_t r = '{s: "r", q: '{k}};
    fork
      begin
        #5;
        $display("child %0d: s=%s q=%0d b=%0d r=%s/%0d", k, s, q.size(), b.v, r.s, r.q.size());
        s = {s, "?"};
      end
      begin
        #7;
        $display("child %0d later: s=%s", k, s);
      end
    join_none
    s = {s, "!"};
    q.push_back(0);
    b.v += 10;
    r.q.push_back(k);
  endtask

  initial begin
    spawn(1);
    spawn(2);
    #10;
    $display("done");
    $finish;
  end
endmodule
