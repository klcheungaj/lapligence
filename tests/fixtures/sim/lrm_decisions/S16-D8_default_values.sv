// Decision S16-D8: each generation of a value-returning production starts
// with the default value of its type, and each implicit variable of a rule
// starts with its type's default value when the rule is generated. A
// production that ends without returning a value therefore yields that
// default, and the implicit variable of a production that the rule did not
// generate (an if or case branch not taken, a repeat count of zero) reads as
// the default rather than a value from an earlier generation. The clause does
// not give the value (llg choice following the automatic scope).
//
// IEEE 1800-2009 18.17 (SystemVerilog-1800-2009.txt L31470-31471):
//   "The randsequence statement creates an automatic scope. All production
//   identifiers are local to the scope."
// IEEE 1800-2009 18.17.7 (L31766-31768):
//   "The return statement assigns the given expression to the corresponding
//   production. The return value can be read in the code blocks of the
//   production that triggered the generation of the production returning a
//   value."
module tb;
  int calls;
  bit cond;
  initial begin
    randsequence (main)
      main : silent first first ;
      silent : iv lv sv { $display("iv=%0d lv=%b sv='%s'", iv, lv, sv); } ;
      int iv : { } ;
      logic [3:0] lv : { } ;
      string sv : { } ;
      first : counted counted { $display("counted=%0d %0d", counted[1], counted[2]); }
              branch ;
      int counted : { calls = calls + 1; if (calls == 1) return 5; } ;
      branch : if (cond) d(5) else d(20) { $display("d=%0d %0d", d[1], d[2]); cond = !cond; } ;
      int d (int value) : { return value; } ;
    endsequence
    $finish;
  end
endmodule
