// A19-D13: statement shapes the diagrams of 37.56-37.73 leave open. A delay
// or event control with an empty body controls a vpiNullStmt; an event list
// written with `or` or `,` is one vpiEventOrOp; `k++`, `--k` and `k += 2`
// are vpiAssignment objects whose vpiOpType names the operator, the first
// two without vpiRhs; unique0 is vpiUniqueQualifier and `case inside` adds
// vpiInsideQualifier; the begin standing for a task body of several
// statements starts at its first statement.
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
