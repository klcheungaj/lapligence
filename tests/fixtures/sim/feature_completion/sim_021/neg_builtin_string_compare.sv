// IEEE 1800-2009 11.11, 6.16: strings already compare with `<`, so this
// relational prototype cannot be declared.
module tb;
  function automatic bit lt(string a, string b);
    return 1;
  endfunction
  bind < function bit lt(string, string);
  string a, b;
  initial $display("%0d", a < b);
endmodule
