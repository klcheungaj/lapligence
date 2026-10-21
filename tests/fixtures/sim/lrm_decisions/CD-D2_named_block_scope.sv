// Decision CD-D2: `%m` inside a named block of a class method names the
// block, and `%m` inside a named fork-join names the fork block, in class
// methods and in modules alike (`tb.top`). An out-of-block (`extern`) method
// body is named like an in-class one.
//
// IEEE 1800-2009 21.2.1.6 (SystemVerilog-1800-2009.txt L36379-36381): "The
// %m format specifier does not accept an argument. Instead, it causes the
// display task to print the hierarchical name of the design element,
// subroutine, named block, or labeled statement that invokes the system task
// containing the format specifier." 9.3.4 L11571-11572: "Both sequential and
// parallel blocks can be named by adding : name_of_block after the keywords
// begin or fork. A named block creates a new hierarchy scope." The text does
// not say how the scope of a class method is spelled before the block name,
// so the class lines print only the trailing component of `%m` (the block
// or method name) through `tail`; the module lines print the whole name,
// which names only design elements and blocks. The unnamed initial block
// declares nothing, so it has no name of its own on any simulator. llg's
// full class-scope names are checked by llg_policy/CD-D2_full_scope_names.
function automatic string tail(string s);
  int e = s.len();
  int i;
  while (e > 0 && s[e - 1] == " ") e--;
  for (i = e - 1; i >= 0; i--) if (s[i] == "." || s[i] == ":") break;
  return s.substr(i + 1, e - 1);
endfunction

class C;
  function void f();
    begin : fb
      $display("f: %s", tail($sformatf("%m")));
    end
  endfunction
  task t();
    fork : tf
      $display("t: %s", tail($sformatf("%m")));
    join
  endtask
  extern function void e();
endclass

function void C::e();
  $display("e: %s", tail($sformatf("%m")));
endfunction

module tb;
  C c;
  task mt();
    fork : mf
      $display("mt: %m");
    join
  endtask
  initial begin
    c = new;
    c.f();
    c.t();
    c.e();
    fork : top
      $display("top: %m");
    join
    mt();
    $finish;
  end
endmodule
