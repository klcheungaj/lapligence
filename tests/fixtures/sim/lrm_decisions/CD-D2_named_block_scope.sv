// Decision CD-D2: `%m` inside a named block of a class method names the
// block below the method (`C.f.fb`, extending S99-D10), and `%m` inside a
// named fork-join names the fork block, in class methods and in modules
// alike (`tb.top`). An out-of-block (`extern`) method body is named like an
// in-class one (`C.e`).
//
// IEEE 1800-2009 21.2.1.6 (SystemVerilog-1800-2009.txt L36379-36381): "The
// %m format specifier does not accept an argument. Instead, it causes the
// display task to print the hierarchical name of the design element,
// subroutine, named block, or labeled statement that invokes the system task
// containing the format specifier." 9.3.4 L11571-11572: "Both sequential and
// parallel blocks can be named by adding : name_of_block after the keywords
// begin or fork. A named block creates a new hierarchy scope." The `tb`
// lines follow from the text (llg formerly printed `tb` and `tb.t` for them);
// the `llg` lines name class code, which is llg policy (S99-D10).
class C;
  function void f();
    begin : fb
      $display("llg f: %m");
    end
  endfunction
  task t();
    fork : tf
      $display("llg t: %m");
    join
  endtask
  extern function void e();
endclass

function void C::e();
  $display("llg e: %m");
endfunction

module tb;
  task mt();
    fork : mf
      $display("mt: %m");
    join
  endtask
  initial begin
    static C c = new;
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
