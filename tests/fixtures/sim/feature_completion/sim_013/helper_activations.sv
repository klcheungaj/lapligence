// SIM-013 A02: event controls evaluated in the activation that reached them.
// An automatic task's qualifier reads its own locals; disabling the waiting
// activation discards its wait and a new call waits with its own values
// (SV 9.4.2.3, 9.6.2, 13.3.2). A const ref helper follows its actual
// (SV 13.5.2). A task waiting on a select of its `ref` formal recurses with
// each activation following the caller's variable; an element actual is
// followed through its qualifier (SV 13.5.2, 9.4.2).
`timescale 1ns / 1ns
module tb;
  int sig = 0;
  int arr[4] = '{1, 2, 3, 4};
  int i = 0;

  function automatic int pick(const ref int x[4], input int k);
    return x[k];
  endfunction

  task automatic watch(input int lim, input string tag);
    int local_lim = lim;
    @(sig iff sig > local_lim);
    $display("%0t %s sig=%0d", $time, tag, sig);
  endtask

  task automatic local_event();
    int a = 0;
    fork
      begin
        @(a iff a == 1);
        $display("%0t local a=%0d", $time, a);
      end
      begin
        #1 a = 2;
        #1 a = 1;
      end
    join
  endtask

  task automatic rise(ref logic [1:0] s, input int n);
    @(posedge s[0]);
    $display("%0t rise level %0d", $time, n);
    if (n != 0) rise(s, n - 1);
  endtask

  task automatic drive_local();
    logic [1:0] l = 2'b00;
    fork
      rise(l, 1);
      begin
        #1 l = 2'b01;
        #1 l = 2'b00;
        #1 l = 2'b11;
      end
    join
  endtask

  task automatic above(ref int x, input int lim);
    @(x iff x > lim);
    $display("%0t above x=%0d", $time, x);
  endtask

  initial #0 forever begin
    @(pick(arr, i));
    $display("%0t pick=%0d", $time, pick(arr, i));
  end

  initial begin
    fork : first
      watch(5, "A");
    join_none
    #1 sig = 3;
    #1 disable first;
    fork
      watch(1, "B");
    join_none
    #1 sig = 4;
    #1 sig = 6;
    #1 arr[1] = 9;
    #1 arr[0] = 7;
    #1 i = 1;
    #1 arr[0] = 5;
    #1 local_event();
    #1 drive_local();
    fork
      above(arr[2], 6);
      begin
        #1 arr[2] = 5;
        #1 arr[3] = 9;
        #1 arr[2] = 7;
      end
    join
    $finish(0);
  end
endmodule
