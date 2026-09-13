module tb;
  task automatic stop_from_branch;
    begin
      $display("CHECK: branch before");
      $finish(0);
      $display("CHECK: branch after");
    end
  endtask

  initial begin
    fork
      begin
        stop_from_branch();
      end
      begin
        #1 $display("CHECK: sibling");
      end
    join_none
    #2 $display("CHECK: parent after");
  end

  final begin
    $display("CHECK: final");
  end
endmodule
