// Static-review counterexample; NOT EXECUTED.
module tb;
  process parent;
  initial begin
    parent = process::self();
    fork
      begin
        #1;
        parent.kill();
        $fatal(1, "killed child continued after killing its ancestor");
      end
    join
    $fatal(1, "killed parent continued");
  end
  initial begin #5; $display("surviving independent process"); $finish(0); end
endmodule
