// A19-D13: statement shapes the diagrams of 37.56-37.73 leave open. A delay
// or event control with an empty body controls a vpiNullStmt; an event list
// written with `or` or `,` is one vpiEventOrOp; `k++`, `--k` and `k += 2`
// are vpiAssignment objects whose vpiOpType names the operator: an
// increment or decrement is a vpiAddOp or vpiSubOp assignment whose vpiRhs
// is the constant 1; unique0 is vpiUniqueQualifier and `case inside` adds
// vpiInsideQualifier. A written block (`begin`, `fork`, named or not) starts
// at the line of its keyword, even when it holds one statement; the begin
// standing for a task or function body of several statements starts at the
// token after the subroutine name (its port list, else the `;` ending the
// header).
//
// IEEE 1800-2009 37.59 detail 1 (SystemVerilog-1800-2009.txt L61124-L61126):
// "vpiOpType shall return vpiAssignmentOp for normal assignments ... For
// assignment operators, vpiOpType shall return a value that corresponds to
// the operator that is combined with the assignment". 37.12 detail 5
// (L58098-L58100): "If the number of statements is greater than 1, the
// vpiStmt relation shall return an unnamed begin that contains the
// statements of the task or function." Neither says how an increment is
// shaped or which line a block or an implicit begin reports: llg choices
// following industry-standard behaviour. Some simulators return a
// vpiNullStmt for a task without statements; llg returns NULL as 37.12 detail
// 5 says: "If the number of statements is zero, the vpiStmt relation shall
// return NULL."
module tb;
  logic a = 0, b = 0, c = 1;
  int k = 0;
  logic [1:0] sel = 2'd1;
  task automatic two;
    k = 1;
    k = 2;
  endtask
  initial begin
    #1;
    @(a, b);
    @(posedge a or negedge b iff c);
    k++;
    --k;
    k += 2;
    if (c)
      begin
        k = 6;
      end
    begin : named
      k = 7;
    end
    unique0 if (a) k = 3;
    unique case (sel) inside
      [0:1]: k = 4;
      default: k = 5;
    endcase
  end
  initial begin
    #2 $d13;
    a = 1;
    #1 b = 1;
    #1 a = 0;
    #1 $finish;
  end
endmodule
