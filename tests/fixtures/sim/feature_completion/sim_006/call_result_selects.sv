module tb;
  typedef int iq_t[$];
  typedef string sq_t[$];
  typedef int aa_t[string];
  int n;
  function automatic iq_t make(int k);
    iq_t q;
    n++;
    for (int i = 0; i < k; i++) q.push_back(i * 2);
    return q;
  endfunction
  function automatic sq_t names();
    names.push_back("a");
    names.push_back("b");
  endfunction
  function automatic aa_t amap();
    amap["x"] = 3;
  endfunction
  class C;
    function iq_t m(); m.push_back(9); endfunction
  endclass
  int x;
  C c;
  initial begin
    c = new;
    x = make(3)[1];
    $display("%0d %0d", x, n);
    $display("%0d", make(4).size());
    if (make(2).size() == 2) $display("size2");
    $display("%s", names()[1]);
    $display("%0d", amap()["x"]);
    $display("%0d", amap().exists("y"));
    $display("%0d", make(4).sum());
    $display("%0d", c.m()[0]);
    x = make(3)[1] + make(4)[3];
    $display("%0d %0d", x, n);
    $finish;
  end
endmodule
