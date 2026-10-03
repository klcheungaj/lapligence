// SV2009 10.9, 10.4.2: frozen record selectors, overlapping targets, static NBA.
module tb;
  typedef logic [7:0] lane_t;
  typedef lane_t pair_t[2];
  typedef struct { bit [7:0] binary; lane_t four; } record_t;
  record_t records[2];
  record_t source;
  logic [11:0] overlapping;
  int selected, observed;
  task automatic persistent();
    static lane_t retained[2];
    retained = '{8'h00,8'h00};
    pair_t'{retained[0],retained[1]} <= '{8'h37,8'h91};
  endtask
  initial begin
    observed=0;
    records[0]='{0,0}; records[1]='{0,0};
    source = '{8'hff,8'hz1};
    selected = 0;
    record_t'{records[selected].binary, records[selected].four} = source;
    if (records[0].binary !== 8'hff || records[0].four !== 8'hz1) $fatal(1,"record destinations");
    overlapping=0;
    pair_t'{overlapping[7:0],overlapping[11:4]} = '{8'haa,8'haa};
    if(overlapping!==12'haaa) $fatal(1,"overlapping targets");
    record_t'{records[selected].binary, records[selected].four} <= source;
    source = '{0,0}; selected=1;
    persistent();
    #1;
    if(records[0].binary!==8'hff || records[0].four!==8'hz1 || records[1].binary!==0) $fatal(1,"record NBA capture");
    if(persistent.retained[0]!==8'h37 || persistent.retained[1]!==8'h91) $fatal(1,"static NBA lifetime");
    if(observed != 1) $fatal(1,"selected cell notification");
    $display("scatter_records=pass");
    $finish(0);
  end
  initial begin
    #0;
    @(persistent.retained[1]);
    observed=1;
  end
endmodule
