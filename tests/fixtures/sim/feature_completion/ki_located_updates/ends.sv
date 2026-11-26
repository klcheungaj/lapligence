// An operand that ends the process, forks, or is an imported function that
// calls back; the assignments after it are made as written (readme: "Other
// operands").
`timescale 1ns / 1ns
module tb;
  typedef struct {
    int a;
    string s;
  } rec_t;
  rec_t ra[3], one;
  int v, w, plain;
  string text;
  import "DPI-C" context function int through_c(input int k);
  export "DPI-C" function from_c;
  function int from_c(input int k);
    v <= k;
    ra[0].s <= $sformatf("c%0d", k);
    return k + 1;
  endfunction
  function int dies(int k);
    w <= k;
    process::self().kill();
    return k;
  endfunction
  function string dies_text(int k);
    process::self().kill();
    return "killed";
  endfunction
  function int spawn(int k);
    fork
      begin
        ra[2].s <= "spawned";
        #0 v <= 50 + k;
      end
    join_none
    return k;
  endfunction
  initial begin
    fork
      begin
        ra[1].a <= dies(1);
        $display("not reached");
      end
      begin
        ra[1].s <= dies_text(2);
        $display("not reached");
      end
      begin
        one.s <= @(plain) dies_text(3);
        $display("not reached");
      end
    join
    // The next updates go where they are written.
    plain <= 5;
    text <= "text";
    #1 $display("%0d '%s' '%s' %0d %0d %s", ra[1].a, ra[1].s, one.s, w, plain, text);
    ra[1].a <= spawn(4);
    #1 $display("%0d %0d %s", ra[1].a, v, ra[2].s);
    ra[1].s <= $sformatf("got%0d", through_c(6));
    ra[2].a <= through_c(7);
    #1 $display("%s %0d %0d %s", ra[1].s, ra[2].a, v, ra[0].s);
    fork
      begin : named
        one.a <= leave(8);
        $display("not reached");
      end
    join
    plain <= 9;
    #1 $display("%0d %0d %0d", one.a, plain, v);
    $finish;
  end
  function int leave(int k);
    v <= k;
    disable tb.named;
    return k;
  endfunction
endmodule
