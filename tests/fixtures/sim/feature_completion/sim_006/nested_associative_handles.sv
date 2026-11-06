module tb;
  class S; int id; function new(int i); id = i; endfunction endclass
  int aa[string][int];
  S hs[string][int];
  function automatic int put();
    aa["x"][2] = 7;
    return 6;
  endfunction
  initial begin
    aa["x"][1] = 1;
    aa["x"][1] = put();
    $display("%0d %0d", aa["x"][1], aa["x"][2]);
    for (int i = 0; i < 2000; i++) begin
      hs["a"][i % 7] = new(i);
      if (i % 3 == 0) hs["b"][i % 5] = hs["a"][i % 7];
    end
    $display("%0d %0d %0d", hs["a"].num(), hs["b"].num(), hs["a"][3].id);
    $finish;
  end
endmodule
