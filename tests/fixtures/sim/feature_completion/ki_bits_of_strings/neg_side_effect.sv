module tb;
  typedef int ints_t [$];
  string s = "abc";
  string list [$] = '{"a", "b"};
  int i;
  function automatic string f(string t); return t; endfunction
  function automatic ints_t q(); return '{1, 2}; endfunction
  initial $display("%0d", $bits(list[i++]));
endmodule
