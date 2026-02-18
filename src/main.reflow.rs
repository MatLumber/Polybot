//!!  PPoollyyBBoott  --  DDiirreeccttiioonnaall  TTrraaddiinngg  BBoott  ffoorr  PPoollyymmaarrkkeett

//!!

//!!  PPrreeddiiccttss  UUPP//DDOOWWNN  ddiirreeccttiioonn  ffoorr  1155mm//11hh  BBTTCC//EETTHH//SSOOLL//XXRRPP  mmaarrkkeettss

//!!  uussiinngg  PPoollyymmaarrkkeett--nnaattiivvee  RRTTDDSS  ++  CCLLOOBB  mmaarrkkeett  ddaattaa..

mod backtesting;
mod clob;
mod config;
mod features;
mod oracle;
mod paper_trading;
mod persistence;
mod polymarket;
mod risk;
mod strategy;
mod types;
#[cfg(feature = "ddaasshhbbooaarrdd""))]]
mmoodd  ddaasshhbbooaarrdd;;
uussee  aannyyhhooww::::RReessuulltt;;
uussee  ssttdd::::ssyynncc::::AArrcc;;
uussee  ttookkiioo::::ssyynncc::::{{
mmppsscc,,  MMuutteexx}}
;;
uussee  ttrraacciinngg::::{{
eerrrroorr,,  iinnffoo,,  wwaarrnn}}
;;
uussee  ttrraacciinngg__ssuubbssccrriibbeerr::::{{
ffmmtt,,  llaayyeerr::::SSuubbssccrriibbeerrEExxtt,,  uuttiill::::SSuubbssccrriibbeerrIInniittEExxtt,,  EEnnvvFFiilltteerr}}
;;
uussee  ccrraattee::::cclloobb::::{{
CClloobbCClliieenntt,,  OOrrddeerr}}
;;
uussee  ccrraattee::::ccoonnffiigg::::AAppppCCoonnffiigg;;
uussee  ccrraattee::::ffeeaattuurreess::::{{
FFeeaattuurreeEEnnggiinnee,,  FFeeaattuurreess,,  MMaarrkkeettRReeggiimmee,,  OOrrddeerrbbooookkIImmbbaallaanncceeTTrraacckkeerr}}
;;
uussee  ccrraattee::::oorraaccllee::::PPrriicceeAAggggrreeggaattoorr;;
uussee  ccrraattee::::ppaappeerr__ttrraaddiinngg::::{{
PPaappeerrTTrraaddiinnggCCoonnffiigg,,  PPaappeerrTTrraaddiinnggEEnnggiinnee}}
;;
uussee  ccrraattee::::ppeerrssiisstteennccee::::{{
BBaallaanncceeTTrraacckkeerr,,  CCssvvPPeerrssiisstteennccee,,  HHaarrddRReesseettOOppttiioonnss}}
;;
uussee  ccrraattee::::rriisskk::::RRiisskkMMaannaaggeerr;;
uussee  ccrraattee::::ssttrraatteeggyy::::{{
SSttrraatteeggyyCCoonnffiigg,,  SSttrraatteeggyyEEnnggiinnee,,  TTrraaddeeRReessuulltt}}
;;
uussee  ccrraattee::::ttyyppeess::::{{
AAsssseett,,  DDiirreeccttiioonn,,  FFeeaattuurreeSSeett,,  PPrriicceeSSoouurrccee,,  PPrriicceeTTiicckk,,  SSiiggnnaall,,  TTiimmeeffrraammee}}
;;
##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]
uussee  ccrraattee::::ddaasshhbbooaarrdd::::{{
PPaappeerrSSttaattssRReessppoonnssee,,  PPoossiittiioonnRReessppoonnssee,,  TTrraaddeeRReessppoonnssee}}
;;
ccoonnsstt  BBOOTT__TTAAGG::  &&ssttrr  ==  eennvv!!((""CCAARRGGOO__PPKKGG__VVEERRSSIIOONN""));;
##[[ttookkiioo::::mmaaiinn]]
aassyynncc  ffnn  mmaaiinn(())  -->>  RReessuulltt<<(())>>  {{
        ///  IInniittiiaalliizzee  llooggggiinngg        iinniitt__llooggggiinngg(())??;;
        iinnffoo!!((                bboott__ttaagg  ==  %%BBOOTT__TTAAGG,,                ""üü§§ññ  PPoollyyBBoott  vv{{
}}
  ssttaarrttiinngg......"",,  BBOOTT__TTAAGG        ));;
        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        iinnffoo!!((""üüññ••ÔÔ∏∏èè  DDaasshhbbooaarrdd  ffeeaattuurree  EENNAABBLLEEDD  --  sseerrvveerr  wwiillll  ssttaarrtt  oonn  ppoorrtt  33000000""));;
        ##[[ccffgg((nnoott((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))))]]        iinnffoo!!((""üüññ••ÔÔ∏∏èè  DDaasshhbbooaarrdd  ffeeaattuurree  DDIISSAABBLLEEDD""));;
        ///  LLooaadd  ccoonnffiigguurraattiioonn        lleett  rruunnttiimmee__aarrggss  ==  ppaarrssee__rruunnttiimmee__aarrggss(())??;;
        lleett  ccoonnffiigg  ==  AAppppCCoonnffiigg::::llooaadd(())??;;
        iinnffoo!!((ccoonnffiigg__ddiiggeesstt  ==  %%ccoonnffiigg..ddiiggeesstt(()),,  ""‚‚úúÖÖ  CCoonnffiigguurraattiioonn  llooaaddeedd""));;
        lleett  __ssttaarrttuupp__rreesseett__eexxeeccuutteedd  ==  mmaayybbee__rruunn__ssttaarrttuupp__rreesseett((&&ccoonnffiigg,,  &&rruunnttiimmee__aarrggss))??;;
        iiff  rruunnttiimmee__aarrggss..rreesseett__mmooddee..iiss__ssoommee(())  {{
                iinnffoo!!((""RReesseett  ccoommmmaanndd  ccoommpplleetteedd;;
  eexxiittiinngg  bbyy  CCLLII  rreeqquueesstt""));;
                rreettuurrnn  OOkk(((())));;
        }}
        ///  VVaalliiddaattee  eennvviirroonnmmeenntt        ccoonnffiigg..vvaalliiddaattee__eennvv(())??;;
        ///  CCrreeaattee  cchhaannnneellss  ffoorr  iinntteerr--ccoommppoonneenntt  ccoommmmuunniiccaattiioonn        lleett  ((pprriiccee__ttxx,,  mmuutt  pprriiccee__rrxx))  ==  mmppsscc::::cchhaannnneell::::<<PPrriicceeTTiicckk>>((11000000));;
        lleett  ((ppaappeerr__pprriiccee__ttxx,,  mmuutt  ppaappeerr__pprriiccee__rrxx))  ==  mmppsscc::::cchhaannnneell::::<<PPrriicceeTTiicckk>>((11000000));;
        lleett  ((ffeeaattuurree__ttxx,,  mmuutt  ffeeaattuurree__rrxx))  ==  mmppsscc::::cchhaannnneell::::<<FFeeaattuurreess>>((550000));;
        lleett  ((ssiiggnnaall__ttxx,,  mmuutt  ssiiggnnaall__rrxx))  ==  mmppsscc::::cchhaannnneell::::<<SSiiggnnaall>>((110000));;
        lleett  ((oorrddeerr__ttxx,,  mmuutt  oorrddeerr__rrxx))  ==  mmppsscc::::cchhaannnneell::::<<OOrrddeerr>>((110000));;
        ///  IInniittiiaalliizzee  ccoommppoonneennttss        lleett  oorraaccllee  ==  AArrcc::::nneeww((PPrriicceeAAggggrreeggaattoorr::::nneeww((55000000,,  22,,  110000))));;
        lleett  ffeeaattuurree__eennggiinnee  ==  AArrcc::::nneeww((MMuutteexx::::nneeww((FFeeaattuurreeEEnnggiinnee::::nneeww(())))));;
        ///  ‚‚îîÄÄ‚‚îîÄÄ  OOrrddeerrbbooookk  IImmbbaallaannccee  TTrraacckkeerr  ((ffoorr  mmiiccrroossttrruuccttuurree  aannaallyyssiiss))  ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ        lleett  oorrddeerrbbooookk__ttrraacckkeerr  ==  AArrcc::::nneeww((ssttdd::::ssyynncc::::MMuutteexx::::nneeww((OOrrddeerrbbooookkIImmbbaallaanncceeTTrraacckkeerr::::nneeww(())))));;
        {{
                lleett  ttrraacckkeerr__cclloonnee  ==  oorrddeerrbbooookk__ttrraacckkeerr..cclloonnee(());;
                ffeeaattuurree__eennggiinnee                        ..lloocckk(())                        ..aawwaaiitt                        ..sseett__oorrddeerrbbooookk__ttrraacckkeerr((ttrraacckkeerr__cclloonnee));;
        }}
        iinnffoo!!((""üüììää  OOrrddeerrbbooookkIImmbbaallaanncceeTTrraacckkeerr  ccoonnnneecctteedd  ttoo  FFeeaattuurreeEEnnggiinnee""));;
        lleett  ssttrraatteeggyy  ==  AArrcc::::nneeww((MMuutteexx::::nneeww((SSttrraatteeggyyEEnnggiinnee::::nneeww((SSttrraatteeggyyCCoonnffiigg::::ddeeffaauulltt(())))))));;
        ///  LLooaadd  ccaalliibbrraattoorr  ssttaattee  ((vv22  pprreeffeerrrreedd,,  vv11  ffaallllbbaacckk))        lleett  ccaalliibbrraattoorr__ssttaattee__ffiillee__vv11  ==                ssttdd::::ppaatthh::::PPaatthhBBuuff::::ffrroomm((&&ccoonnffiigg..ppeerrssiisstteennccee..ddaattaa__ddiirr))..jjooiinn((""ccaalliibbrraattoorr__ssttaattee..jjssoonn""));;
        lleett  ccaalliibbrraattoorr__ssttaattee__ffiillee__vv22  ==                ssttdd::::ppaatthh::::PPaatthhBBuuff::::ffrroomm((&&ccoonnffiigg..ppeerrssiisstteennccee..ddaattaa__ddiirr))..jjooiinn((""ccaalliibbrraattoorr__ssttaattee__vv22..jjssoonn""));;
        lleett  mmuutt  llooaaddeedd__ccaalliibbrraattoorr__ssttaattee  ==  ffaallssee;;
        iiff  ccaalliibbrraattoorr__ssttaattee__ffiillee__vv22..eexxiissttss(())  {{
                mmaattcchh  ssttdd::::ffss::::rreeaadd__ttoo__ssttrriinngg((&&ccaalliibbrraattoorr__ssttaattee__ffiillee__vv22))  {{
                        OOkk((jjssoonn))  ==>>  mmaattcchh  sseerrddee__jjssoonn::::ffrroomm__ssttrr::::<<                                ssttdd::::ccoolllleeccttiioonnss::::HHaasshhMMaapp<<SSttrriinngg,,  VVeecc<<ssttrraatteeggyy::::IInnddiiccaattoorrSSttaattss>>>>,,                        >>((&&jjssoonn))                        {{
                                OOkk((ssttaattss__bbyy__mmaarrkkeett))  ==>>  {{
                                        ssttrraatteeggyy                                                ..lloocckk(())                                                ..aawwaaiitt                                                ..iimmppoorrtt__ccaalliibbrraattoorr__ssttaattee__vv22((ssttaattss__bbyy__mmaarrkkeett));;
                                        llooaaddeedd__ccaalliibbrraattoorr__ssttaattee  ==  ttrruuee;;
                                        iinnffoo!!((                                                ppaatthh  ==  %%ccaalliibbrraattoorr__ssttaattee__ffiillee__vv22..ddiissppllaayy(()),,                                                ""LLooaaddeedd  ccaalliibbrraattoorr  vv22  ssttaattee  ffrroomm  ddiisskk""                                        ));;
                                }}
                                EErrrr((ee))  ==>>  wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  ppaarrssee  ccaalliibbrraattoorr  vv22  ssttaattee"")),,                        }}
,,                        EErrrr((ee))  ==>>  wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  rreeaadd  ccaalliibbrraattoorr  vv22  ssttaattee  ffiillee"")),,                }}
        }}
  eellssee  iiff  ccaalliibbrraattoorr__ssttaattee__ffiillee__vv11..eexxiissttss(())  {{
                mmaattcchh  ssttdd::::ffss::::rreeaadd__ttoo__ssttrriinngg((&&ccaalliibbrraattoorr__ssttaattee__ffiillee__vv11))  {{
                        OOkk((jjssoonn))  ==>>  mmaattcchh  sseerrddee__jjssoonn::::ffrroomm__ssttrr::::<<VVeecc<<ssttrraatteeggyy::::IInnddiiccaattoorrSSttaattss>>>>((&&jjssoonn))  {{
                                OOkk((ssttaattss))  ==>>  {{
                                        lleett  ttoottaall::  uussiizzee  ==  ssttaattss..iitteerr(())..mmaapp((||ss||  ss..ttoottaall__ssiiggnnaallss))..mmaaxx(())..uunnwwrraapp__oorr((00));;
                                        ssttrraatteeggyy..lloocckk(())..aawwaaiitt..iimmppoorrtt__ccaalliibbrraattoorr__ssttaattee((ssttaattss));;
                                        llooaaddeedd__ccaalliibbrraattoorr__ssttaattee  ==  ttrruuee;;
                                        iinnffoo!!((                                                ttrraaddeess  ==  ttoottaall,,                                                ppaatthh  ==  %%ccaalliibbrraattoorr__ssttaattee__ffiillee__vv11..ddiissppllaayy(()),,                                                ""LLooaaddeedd  lleeggaaccyy  ccaalliibbrraattoorr  vv11  ssttaattee  ffrroomm  ddiisskk""                                        ));;
                                }}
                                EErrrr((ee))  ==>>  wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  ppaarrssee  ccaalliibbrraattoorr  vv11  ssttaattee"")),,                        }}
,,                        EErrrr((ee))  ==>>  wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  rreeaadd  ccaalliibbrraattoorr  vv11  ssttaattee  ffiillee"")),,                }}
        }}
  eellssee  {{
                iinnffoo!!((""NNoo  ccaalliibbrraattoorr  ssttaattee  ffiillee  ffoouunndd,,  ssttaarrttiinngg  wwiitthh  ffrreesshh  mmaarrkkeett  lleeaarrnniinngg""));;
        }}
        lleett  rriisskk__mmaannaaggeerr  ==  AArrcc::::nneeww((RRiisskkMMaannaaggeerr::::nneeww((rriisskk::::RRiisskkCCoonnffiigg::::ddeeffaauulltt(())))));;
        lleett  ddrryy__rruunn  ==  ccoonnffiigg..bboott..ddrryy__rruunn;;
        lleett  cclloobb__cclliieenntt  ==  AArrcc::::nneeww((CClloobbCClliieenntt::::wwiitthh__ddrryy__rruunn((ccoonnffiigg..eexxeeccuuttiioonn..cclloonnee(()),,  ddrryy__rruunn))));;
        lleett  ppaappeerr__ttrraaddiinngg__eennaabblleedd  ==  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..eennaabblleedd;;
        iiff  ppaappeerr__ttrraaddiinngg__eennaabblleedd  {{
                iinnffoo!!((                        ""üüììãã  PPAAPPEERR  TTRRAADDIINNGG  mmooddee  eennaabblleedd  --  vviirrttuuaall  bbaallaannccee::  $${{
::..22}}
"",,                        ccoonnffiigg..ppaappeerr__ttrraaddiinngg..iinniittiiaall__bbaallaannccee                ));;
        }}
  eellssee  iiff  ddrryy__rruunn  {{
                iinnffoo!!((""üüßß™™  DDRRYY__RRUUNN  mmooddee  eennaabblleedd  --  nnoo  rreeaall  oorrddeerrss  wwiillll  bbee  ssuubbmmiitttteedd""));;
        }}
  eellssee  {{
                wwaarrnn!!((""‚‚öö††ÔÔ∏∏èè  LLIIVVEE  mmooddee  eennaabblleedd  --  rreeaall  oorrddeerrss  wwiillll  bbee  ssuubbmmiitttteedd!!""));;
        }}
        lleett  ccssvv__ppeerrssiisstteennccee  ==  AArrcc::::nneeww((CCssvvPPeerrssiisstteennccee::::nneeww((&&ccoonnffiigg..ppeerrssiisstteennccee..ddaattaa__ddiirr))??));;
        lleett  bbaallaannccee__ttrraacckkeerr  ==  AArrcc::::nneeww((BBaallaanncceeTTrraacckkeerr::::nneeww(())));;
        iiff  !!llooaaddeedd__ccaalliibbrraattoorr__ssttaattee  {{
                iinnffoo!!((""FFrreesshh  ccaalliibbrraattoorr  ssttaarrtt::  bboooottssttrraapp  ffrroomm  BBiinnaannccee  iiss  ddiissaabblleedd""));;
        }}
        ///  ‚‚îîÄÄ‚‚îîÄÄ  DDaasshhbbooaarrdd  AAPPII  ((ooppttiioonnaall,,  oonnllyy  wwiitthh  ffeeaattuurree  ffllaagg))  ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        lleett  ((ddaasshhbbooaarrdd__mmeemmoorryy,,  ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr))  ==  {{
                lleett  mmeemmoorryy  ==  ssttdd::::ssyynncc::::AArrcc::::nneeww((ddaasshhbbooaarrdd::::DDaasshhbbooaarrddMMeemmoorryy::::nneeww((                        ccoonnffiigg..ppaappeerr__ttrraaddiinngg..iinniittiiaall__bbaallaannccee,,                ))));;
                lleett  bbrrooaaddccaasstteerr  ==  ddaasshhbbooaarrdd::::WWeebbSSoocckkeettBBrrooaaddccaasstteerr::::nneeww((110000));;
                ((mmeemmoorryy,,  bbrrooaaddccaasstteerr))        }}
;;
        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        {{
                lleett  ssttrraatt  ==  ssttrraatteeggyy..lloocckk(())..aawwaaiitt;;
                lleett  ssnnaappsshhoott  ==  ssttrraatt..eexxppoorrtt__ccaalliibbrraattoorr__ssttaattee__vv22(());;
                lleett  qquuaalliittyy  ==  ssttrraatt..eexxppoorrtt__ccaalliibbrraattiioonn__qquuaalliittyy__bbyy__mmaarrkkeett(());;
                ddrroopp((ssttrraatt));;
                ddaasshhbbooaarrdd__mmeemmoorryy..sseett__mmaarrkkeett__lleeaarrnniinngg__ssttaattss((ssnnaappsshhoott))..aawwaaiitt;;
                ddaasshhbbooaarrdd__mmeemmoorryy                        ..sseett__ccaalliibbrraattiioonn__qquuaalliittyy__ssttaattss((qquuaalliittyy))                        ..aawwaaiitt;;
        }}
        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        lleett  ddaasshhbbooaarrdd__hhaannddllee  ==  {{
                lleett  mmeemmoorryy  ==  ddaasshhbbooaarrdd__mmeemmoorryy..cclloonnee(());;
                lleett  bbrrooaaddccaasstteerr  ==  ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr..cclloonnee(());;
                lleett  ccssvv__ppeerrssiisstteennccee__cclloonnee  ==  ccssvv__ppeerrssiisstteennccee..cclloonnee(());;
                lleett  rreesseett__eexxeeccuutteedd  ==  __ssttaarrttuupp__rreesseett__eexxeeccuutteedd;;
                ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                        iinnffoo!!((""DDaasshhbbooaarrdd  ssppaawwnn  ssttaarrtteedd  --  iinniittiiaalliizziinngg  sseerrvveerr......""));;
                        iiff  rreesseett__eexxeeccuutteedd  {{
                                iinnffoo!!((""[[11//66]]  SSttaarrttuupp  rreesseett  eexxeeccuutteedd;;
  sskkiippppiinngg  ddaasshhbbooaarrdd  hhiissttoorriiccaall  bboooottssttrraapp""));;
                        }}
  eellssee  {{
                                ///  LLooaadd  hhiissttoorriiccaall  ppaappeerr  ttrraaddeess  oonn  ssttaarrttuupp                                iinnffoo!!((""[[11//66]]  LLooaaddiinngg  hhiissttoorriiccaall  ppaappeerr  ttrraaddeess......""));;
                                mmaattcchh  ccssvv__ppeerrssiisstteennccee__cclloonnee..llooaadd__rreecceenntt__ppaappeerr__ttrraaddeess((1100__000000))  {{
                                        OOkk((ttrraaddeess))  ==>>  {{
                                                iinnffoo!!((""[[22//66]]  LLooaaddeedd  {{
}}
  ttrraaddeess  ffrroomm  CCSSVV"",,  ttrraaddeess..lleenn(())));;
                                                iiff  !!ttrraaddeess..iiss__eemmppttyy(())  {{
                                                        iinnffoo!!((""LLooaaddeedd  {{
}}
  hhiissttoorriiccaall  ppaappeerr  ttrraaddeess"",,  ttrraaddeess..lleenn(())));;
                                                        mmeemmoorryy..sseett__ppaappeerr__ttrraaddeess((ttrraaddeess))..aawwaaiitt;;
                                                        iinnffoo!!((""[[22bb//66]]  sseett__ppaappeerr__ttrraaddeess  ccoommpplleetteedd""));;
                                                }}
                                        }}
                                        EErrrr((ee))  ==>>  {{
                                                wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  llooaadd  hhiissttoorriiccaall  ppaappeerr  ttrraaddeess""));;
                                        }}
                                }}
                                iinnffoo!!((""[[33//66]]  LLooaaddiinngg  rreecceenntt  BBTTCC//EETTHH  pprriiccee  hhiissttoorryy......""));;
                                mmaattcchh  ccssvv__ppeerrssiisstteennccee__cclloonnee                                        ..llooaadd__rreecceenntt__pprriiccee__hhiissttoorryy((&&[[AAsssseett::::BBTTCC,,  AAsssseett::::EETTHH]],,  8866__440000))                                {{
                                        OOkk((rroowwss))  ==>>  {{
                                                iinnffoo!!((""[[44//66]]  LLooaaddeedd  {{
}}
  rroowwss  ffoorr  cchhaarrtt  bboooottssttrraapp"",,  rroowwss..lleenn(())));;
                                                ffoorr  ((aasssseett,,  ttiimmeessttaammpp,,  pprriiccee,,  ssoouurrccee))  iinn  rroowwss  {{
                                                        mmeemmoorryy                                                                ..sseeeedd__pprriiccee__hhiissttoorryy__ppooiinntt((aasssseett,,  pprriiccee,,  ssoouurrccee,,  ttiimmeessttaammpp))                                                                ..aawwaaiitt;;
                                                }}
                                        }}
                                        EErrrr((ee))  ==>>  {{
                                                wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  llooaadd  rreecceenntt  pprriiccee  hhiissttoorryy  ffoorr  ddaasshhbbooaarrdd  bboooottssttrraapp""));;
                                        }}
                                }}
                        }}
                        ///  HHeeaarrttbbeeaatt  hheellppss  tthhee  ffrroonntteenndd  ddeetteecctt  ssttaallee  ccoonnnneeccttiioonnss..                        lleett  hheeaarrttbbeeaatt__bbrrooaaddccaasstteerr  ==  bbrrooaaddccaasstteerr..cclloonnee(());;
                        ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                                lleett  mmuutt  iinntteerrvvaall  ==  ttookkiioo::::ttiimmee::::iinntteerrvvaall((ttookkiioo::::ttiimmee::::DDuurraattiioonn::::ffrroomm__sseeccss((1100))));;
                                lloooopp  {{
                                        iinntteerrvvaall..ttiicckk(())..aawwaaiitt;;
                                        hheeaarrttbbeeaatt__bbrrooaaddccaasstteerr..bbrrooaaddccaasstt__hheeaarrttbbeeaatt(());;
                                }}
                        }}
));;
                        iinnffoo!!((""[[55//66]]  SSttaarrttiinngg  ddaasshhbbooaarrdd  sseerrvveerr  oonn  ppoorrtt  33000000......""));;
                        mmaattcchh  ddaasshhbbooaarrdd::::ssttaarrtt__sseerrvveerr((mmeemmoorryy,,  bbrrooaaddccaasstteerr,,  33000000))..aawwaaiitt  {{
                                OOkk(((())))  ==>>  {{
                                        iinnffoo!!((""[[66//66]]  ssttaarrtt__sseerrvveerr  rreettuurrnneedd  ssuucccceessssffuullllyy  ((uunneexxppeecctteedd))""));;
                                }}
                                EErrrr((ee))  ==>>  {{
                                        eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""DDaasshhbbooaarrdd  sseerrvveerr  ffaaiilleedd  ttoo  ssttaarrtt""));;
                                }}
                        }}
                }}
))        }}
;;
        ///  PPaappeerr  ttrraaddiinngg  eennggiinnee  ((oonnllyy  uusseedd  wwhheenn  ppaappeerr__ttrraaddiinngg..eennaabblleedd  ==  ttrruuee))        ///  CCrreeaattee  sshhaarree  pprriiccee  pprroovviiddeerr  tthhaatt  wwiillll  bbee  ppooppuullaatteedd  bbyy  oorrddeerrbbooookk  ffeeeedd        lleett  ppoollyymmaarrkkeett__sshhaarree__pprriicceess  ==  ssttdd::::ssyynncc::::AArrcc::::nneeww((ppaappeerr__ttrraaddiinngg::::PPoollyymmaarrkkeettSShhaarreePPrriicceess::::nneeww(())));;
        lleett  sshhaarree__pprriicceess__ffoorr__oorrddeerrbbooookk  ==  ppoollyymmaarrkkeett__sshhaarree__pprriicceess..cclloonnee(());;
        lleett  ppaappeerr__eennggiinnee  ==  iiff  ppaappeerr__ttrraaddiinngg__eennaabblleedd  {{
                lleett  pptt__ccoonnffiigg  ==  PPaappeerrTTrraaddiinnggCCoonnffiigg  {{
                        iinniittiiaall__bbaallaannccee::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..iinniittiiaall__bbaallaannccee,,                        sslliippppaaggee__bbppss::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..sslliippppaaggee__bbppss,,                        ffeeee__bbppss::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..ffeeee__bbppss,,                        ttrraaiilliinngg__ssttoopp__ppcctt::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..ttrraaiilliinngg__ssttoopp__ppcctt,,                        ttaakkee__pprrooffiitt__ppcctt::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..ttaakkee__pprrooffiitt__ppcctt,,                        mmaaxx__hhoolldd__dduurraattiioonn__mmss::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..mmaaxx__hhoolldd__dduurraattiioonn__mmss,,                        ddaasshhbbooaarrdd__iinntteerrvvaall__sseeccss::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..ddaasshhbbooaarrdd__iinntteerrvvaall__sseeccss,,                        pprreeffeerr__cchhaaiinnlliinnkk::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..pprreeffeerr__cchhaaiinnlliinnkk,,                        nnaattiivvee__oonnllyy::  ccoonnffiigg                                ..ppoollyymmaarrkkeett                                ..ddaattaa__mmooddee                                ..eeqq__iiggnnoorree__aasscciiii__ccaassee((""nnaattiivvee__oonnllyy"")),,                        cchheecckkppooiinntt__aarrmm__rrooii::  ccoonnffiigg..rriisskk..cchheecckkppooiinntt__aarrmm__rrooii,,                        cchheecckkppooiinntt__iinniittiiaall__fflloooorr__rrooii::  ccoonnffiigg..rriisskk..cchheecckkppooiinntt__iinniittiiaall__fflloooorr__rrooii,,                        cchheecckkppooiinntt__ttrraaiill__ggaapp__rrooii::  ccoonnffiigg..rriisskk..cchheecckkppooiinntt__ttrraaiill__ggaapp__rrooii,,                        hhaarrdd__ssttoopp__rrooii::  ccoonnffiigg..rriisskk..hhaarrdd__ssttoopp__rrooii,,                        ttiimmee__ssttoopp__sseeccoonnddss__ttoo__eexxppiirryy::  ccoonnffiigg..rriisskk..ttiimmee__ssttoopp__sseeccoonnddss__ttoo__eexxppiirryy,,                        kkeellllyy__eennaabblleedd::  ccoonnffiigg..kkeellllyy..eennaabblleedd,,                        kkeellllyy__ffrraaccttiioonn__1155mm::  ccoonnffiigg..kkeellllyy..ffrraaccttiioonn__1155mm,,                        kkeellllyy__ffrraaccttiioonn__11hh::  ccoonnffiigg..kkeellllyy..ffrraaccttiioonn__11hh,,                        kkeellllyy__ccaapp__1155mm::  ccoonnffiigg..kkeellllyy..mmaaxx__bbaannkkrroollll__ffrraaccttiioonn__1155mm,,                        kkeellllyy__ccaapp__11hh::  ccoonnffiigg..kkeellllyy..mmaaxx__bbaannkkrroollll__ffrraaccttiioonn__11hh,,                }}
;;
                ///  SSttaattee  ffiillee  ppaatthh  ffoorr  ppeerrssiisstteennccee                lleett  ssttaattee__ffiillee  ==                        ssttdd::::ppaatthh::::PPaatthhBBuuff::::ffrroomm((&&ccoonnffiigg..ppeerrssiisstteennccee..ddaattaa__ddiirr))..jjooiinn((""ppaappeerr__ttrraaddiinngg__ssttaattee..jjssoonn""));;
                lleett  eennggiinnee  ==  PPaappeerrTTrraaddiinnggEEnnggiinnee::::nneeww((pptt__ccoonnffiigg))                        ..wwiitthh__ppeerrssiisstteennccee((ccssvv__ppeerrssiisstteennccee..cclloonnee(())))                        ..wwiitthh__ssttaattee__ffiillee((ssttaattee__ffiillee))                        ..wwiitthh__ppoollyymmaarrkkeett__sshhaarree__pprriicceess((ppoollyymmaarrkkeett__sshhaarree__pprriicceess..cclloonnee(())));;
                ///  LLooaadd  pprreevviioouuss  ssttaattee  iiff  eexxiissttss                lleett  eennggiinnee  ==  AArrcc::::nneeww((eennggiinnee));;
                iiff  lleett  EErrrr((ee))  ==  eennggiinnee..llooaadd__ssttaattee(())  {{
                        wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  llooaadd  ppaappeerr  ttrraaddiinngg  ssttaattee,,  ssttaarrttiinngg  ffrreesshh""));;
                }}
                iinnffoo!!((""üüììãã  [[PPAAPPEERR]]  CCoonnnneecctteedd  ttoo  rreeaall  PPoollyymmaarrkkeett  sshhaarree  pprriicceess  vviiaa  oorrddeerrbbooookk  ffeeeedd""));;
                ///  ‚‚îîÄÄ‚‚îîÄÄ  CCoonnnneecctt  ccaalliibbrraattiioonn  ccaallllbbaacckk  vviiaa  cchhaannnneell  ‚‚îîÄÄ‚‚îîÄÄ                ///  TThhiiss  aalllloowwss  tthhee  ppaappeerr  eennggiinnee  ttoo  uuppddaattee  iinnddiiccaattoorr  wweeiigghhttss  wwhheenn  ttrraaddeess  cclloossee                ///  WWee  uussee  aa  cchhaannnneell  bbeeccaauussee  tthhee  ccaallllbbaacckk  iiss  ssyynncc  bbuutt  ssttrraatteeggyy  uusseess  aassyynncc  MMuutteexx                lleett  ((ccaalliibbrraattiioonn__ttxx,,  mmuutt  ccaalliibbrraattiioonn__rrxx))  ==                        mmppsscc::::cchhaannnneell::::<<((AAsssseett,,  TTiimmeeffrraammee,,  VVeecc<<SSttrriinngg>>,,  bbooooll,,  ff6644))>>((110000));;
                lleett  ssttrraatteeggyy__ffoorr__ccaalliibbrraattiioonn  ==  ssttrraatteeggyy..cclloonnee(());;
                lleett  ccaalliibbrraattoorr__ssaavvee__ppaatthh  ==  ccaalliibbrraattoorr__ssttaattee__ffiillee__vv22..cclloonnee(());;
                ///  SSppaawwnn  ttaasskk  ttoo  pprroocceessss  ccaalliibbrraattiioonn  eevveennttss  aanndd  ppeerrssiisstt  ttoo  ddiisskk                ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                        wwhhiillee  lleett  SSoommee((((aasssseett,,  ttiimmeeffrraammee,,  iinnddiiccaattoorrss,,  iiss__wwiinn,,  pp__mmooddeell))))  ==                                ccaalliibbrraattiioonn__rrxx..rreeccvv(())..aawwaaiitt                        {{
                                lleett  rreessuulltt  ==  iiff  iiss__wwiinn  {{
                                        TTrraaddeeRReessuulltt::::WWiinn                                }}
  eellssee  {{
                                        TTrraaddeeRReessuulltt::::LLoossss                                }}
;;
                                lleett  mmuutt  ss  ==  ssttrraatteeggyy__ffoorr__ccaalliibbrraattiioonn..lloocckk(())..aawwaaiitt;;
                                ss..rreeccoorrdd__ttrraaddee__wwiitthh__iinnddiiccaattoorrss__ffoorr__mmaarrkkeett((aasssseett,,  ttiimmeeffrraammee,,  &&iinnddiiccaattoorrss,,  rreessuulltt));;
                                ss..rreeccoorrdd__pprreeddiiccttiioonn__oouuttccoommee__ffoorr__mmaarrkkeett((aasssseett,,  ttiimmeeffrraammee,,  pp__mmooddeell,,  iiss__wwiinn));;
                                ///  SSaavvee  ccaalliibbrraattoorr  ssttaattee  ttoo  ddiisskk  aafftteerr  eeaacchh  ttrraaddee                                lleett  ssttaattss  ==  ss..eexxppoorrtt__ccaalliibbrraattoorr__ssttaattee__vv22(());;
                                lleett  ttoottaall__ttrraaddeess  ==  ss..ccaalliibbrraattoorr__ttoottaall__ttrraaddeess(());;
                                lleett  iiss__ccaalliibbrraatteedd  ==  ss..iiss__ccaalliibbrraatteedd(());;
                                ddrroopp((ss));;
                                ///  PPeerrssiisstt  ttoo  JJSSOONN  ffiillee                                mmaattcchh  sseerrddee__jjssoonn::::ttoo__ssttrriinngg__pprreettttyy((&&ssttaattss))  {{
                                        OOkk((jjssoonn))  ==>>  {{
                                                iiff  lleett  EErrrr((ee))  ==  ssttdd::::ffss::::wwrriittee((&&ccaalliibbrraattoorr__ssaavvee__ppaatthh,,  &&jjssoonn))  {{
                                                        wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  ssaavvee  ccaalliibbrraattoorr  ssttaattee""));;
                                                }}
                                        }}
                                        EErrrr((ee))  ==>>  wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  sseerriiaalliizzee  ccaalliibbrraattoorr  ssttaattee"")),,                                }}
                                iinnffoo!!((                                        aasssseett  ==  ??aasssseett,,                                        ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                        iiss__wwiinn  ==  iiss__wwiinn,,                                        pp__mmooddeell  ==  pp__mmooddeell,,                                        iinnddiiccaattoorrss__ccoouunntt  ==  iinnddiiccaattoorrss..lleenn(()),,                                        ttoottaall__ttrraaddeess  ==  ttoottaall__ttrraaddeess,,                                        ccaalliibbrraatteedd  ==  iiss__ccaalliibbrraatteedd,,                                        ""üüßß††  [[CCAALLIIBBRRAATTIIOONN]]  RReeccoorrddeedd  ttrraaddee  rreessuulltt  &&  ssaavveedd  ttoo  ddiisskk""                                ));;
                        }}
                }}
));;
                lleett  ccaalliibbrraattiioonn__ccaallllbbaacckk::  ssttdd::::ssyynncc::::AArrcc<<ppaappeerr__ttrraaddiinngg::::CCaalliibbrraattiioonnCCaallllbbaacckk>>  ==                        ssttdd::::ssyynncc::::AArrcc::::nneeww((BBooxx::::nneeww((                                mmoovvee  ||__aasssseett::  AAsssseett,,                                            ttiimmeeffrraammee::  TTiimmeeffrraammee,,                                            iinnddiiccaattoorrss::  &&[[SSttrriinngg]],,                                            iiss__wwiinn::  bbooooll,,                                            pp__mmooddeell::  ff6644||  {{
                                        lleett  __  ==  ccaalliibbrraattiioonn__ttxx..ttrryy__sseenndd((((                                                __aasssseett,,                                                ttiimmeeffrraammee,,                                                iinnddiiccaattoorrss..ttoo__vveecc(()),,                                                iiss__wwiinn,,                                                pp__mmooddeell,,                                        ))));;
                                }}
,,                        ))  aass  ppaappeerr__ttrraaddiinngg::::CCaalliibbrraattiioonnCCaallllbbaacckk));;
                ///  RReeccrreeaattee  eennggiinnee  wwiitthh  ccaallllbbaacckk                lleett  pptt__ccoonnffiigg  ==  PPaappeerrTTrraaddiinnggCCoonnffiigg  {{
                        iinniittiiaall__bbaallaannccee::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..iinniittiiaall__bbaallaannccee,,                        sslliippppaaggee__bbppss::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..sslliippppaaggee__bbppss,,                        ffeeee__bbppss::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..ffeeee__bbppss,,                        ttrraaiilliinngg__ssttoopp__ppcctt::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..ttrraaiilliinngg__ssttoopp__ppcctt,,                        ttaakkee__pprrooffiitt__ppcctt::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..ttaakkee__pprrooffiitt__ppcctt,,                        mmaaxx__hhoolldd__dduurraattiioonn__mmss::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..mmaaxx__hhoolldd__dduurraattiioonn__mmss,,                        ddaasshhbbooaarrdd__iinntteerrvvaall__sseeccss::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..ddaasshhbbooaarrdd__iinntteerrvvaall__sseeccss,,                        pprreeffeerr__cchhaaiinnlliinnkk::  ccoonnffiigg..ppaappeerr__ttrraaddiinngg..pprreeffeerr__cchhaaiinnlliinnkk,,                        nnaattiivvee__oonnllyy::  ccoonnffiigg                                ..ppoollyymmaarrkkeett                                ..ddaattaa__mmooddee                                ..eeqq__iiggnnoorree__aasscciiii__ccaassee((""nnaattiivvee__oonnllyy"")),,                        cchheecckkppooiinntt__aarrmm__rrooii::  ccoonnffiigg..rriisskk..cchheecckkppooiinntt__aarrmm__rrooii,,                        cchheecckkppooiinntt__iinniittiiaall__fflloooorr__rrooii::  ccoonnffiigg..rriisskk..cchheecckkppooiinntt__iinniittiiaall__fflloooorr__rrooii,,                        cchheecckkppooiinntt__ttrraaiill__ggaapp__rrooii::  ccoonnffiigg..rriisskk..cchheecckkppooiinntt__ttrraaiill__ggaapp__rrooii,,                        hhaarrdd__ssttoopp__rrooii::  ccoonnffiigg..rriisskk..hhaarrdd__ssttoopp__rrooii,,                        ttiimmee__ssttoopp__sseeccoonnddss__ttoo__eexxppiirryy::  ccoonnffiigg..rriisskk..ttiimmee__ssttoopp__sseeccoonnddss__ttoo__eexxppiirryy,,                        kkeellllyy__eennaabblleedd::  ccoonnffiigg..kkeellllyy..eennaabblleedd,,                        kkeellllyy__ffrraaccttiioonn__1155mm::  ccoonnffiigg..kkeellllyy..ffrraaccttiioonn__1155mm,,                        kkeellllyy__ffrraaccttiioonn__11hh::  ccoonnffiigg..kkeellllyy..ffrraaccttiioonn__11hh,,                        kkeellllyy__ccaapp__1155mm::  ccoonnffiigg..kkeellllyy..mmaaxx__bbaannkkrroollll__ffrraaccttiioonn__1155mm,,                        kkeellllyy__ccaapp__11hh::  ccoonnffiigg..kkeellllyy..mmaaxx__bbaannkkrroollll__ffrraaccttiioonn__11hh,,                }}
;;
                lleett  ssttaattee__ffiillee  ==                        ssttdd::::ppaatthh::::PPaatthhBBuuff::::ffrroomm((&&ccoonnffiigg..ppeerrssiisstteennccee..ddaattaa__ddiirr))..jjooiinn((""ppaappeerr__ttrraaddiinngg__ssttaattee..jjssoonn""));;
                lleett  mmuutt  nneeww__eennggiinnee  ==  PPaappeerrTTrraaddiinnggEEnnggiinnee::::nneeww((pptt__ccoonnffiigg))                        ..wwiitthh__ppeerrssiisstteennccee((ccssvv__ppeerrssiisstteennccee..cclloonnee(())))                        ..wwiitthh__ssttaattee__ffiillee((ssttaattee__ffiillee))                        ..wwiitthh__ppoollyymmaarrkkeett__sshhaarree__pprriicceess((ppoollyymmaarrkkeett__sshhaarree__pprriicceess..cclloonnee(())))                        ..wwiitthh__ccaalliibbrraattiioonn__ccaallllbbaacckk((ccaalliibbrraattiioonn__ccaallllbbaacckk));;
                ///  CCooppyy  ssttaattee  ffrroomm  oolldd  eennggiinnee                iiff  lleett  EErrrr((ee))  ==  nneeww__eennggiinnee..llooaadd__ssttaattee(())  {{
                        wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  llooaadd  ppaappeerr  ttrraaddiinngg  ssttaattee  iinn  nneeww  eennggiinnee""));;
                }}
                SSoommee((AArrcc::::nneeww((nneeww__eennggiinnee))))        }}
  eellssee  {{
                NNoonnee        }}
;;
        iinnffoo!!((""‚‚úúÖÖ  AAllll  ccoommppoonneennttss  iinniittiiaalliizzeedd""));;
        ///  ‚‚îîÄÄ‚‚îîÄÄ  PPoollyymmaarrkkeett  OOrrddeerrbbooookk  FFeeeedd  ((WWeebbSSoocckkeett))  ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ        ///  CCoonnnneeccttss  ttoo  PPoollyymmaarrkkeett''ss  WWeebbSSoocckkeett  ttoo  rreecceeiivvee  rreeaall--ttiimmee  oorrddeerrbbooookk  ddaattaa        ///  ffoorr  BBTTCC//EETTHH  mmaarrkkeettss..  TThhiiss  ppooppuullaatteess  tthhee  OOrrddeerrbbooookkIImmbbaallaanncceeTTrraacckkeerr        ///  AANNDD  pprroovviiddeess  rreeaall  sshhaarree  pprriicceess  ffoorr  ppaappeerr  ttrraaddiinngg..        lleett  oorrddeerrbbooookk__ffeeeedd__ttrraacckkeerr  ==  oorrddeerrbbooookk__ttrraacckkeerr..cclloonnee(());;
        lleett  oorrddeerrbbooookk__ffeeeedd__cclliieenntt  ==  cclloobb__cclliieenntt..cclloonnee(());;
        lleett  oorrddeerrbbooookk__sshhaarree__pprriicceess  ==  sshhaarree__pprriicceess__ffoorr__oorrddeerrbbooookk..cclloonnee(());;
        lleett  oorrddeerrbbooookk__ffeeeedd__hhaannddllee  ==  ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                uussee  ccrraattee::::cclloobb::::{{
MMaarrkkeettFFeeeeddCClliieenntt,,  WWssEEvveenntt,,  WWssMMeessssaaggee}}
;;
                uussee  ssttdd::::ccoolllleeccttiioonnss::::HHaasshhMMaapp  aass  SSttddHHaasshhMMaapp;;
                ///  WWaaiitt  aa  bbiitt  ffoorr  mmaarrkkeettss  ttoo  bbee  llooaaddeedd                ttookkiioo::::ttiimmee::::sslleeeepp((ttookkiioo::::ttiimmee::::DDuurraattiioonn::::ffrroomm__sseeccss((55))))..aawwaaiitt;;
                ///  FFeettcchh  ttookkeenn  IIDDss  ffoorr  BBTTCC//EETTHH  mmaarrkkeettss  aanndd  bbuuiilldd  ttookkeenn__iidd  ‚‚ÜÜíí  ((AAsssseett,,  TTiimmeeffrraammee))  mmaappppiinngg                lleett  mmuutt  ttookkeenn__iiddss::  VVeecc<<SSttrriinngg>>  ==  VVeecc::::nneeww(());;
                lleett  mmuutt  ttookkeenn__mmaapp::  SSttddHHaasshhMMaapp<<SSttrriinngg,,  ((AAsssseett,,  TTiimmeeffrraammee,,  DDiirreeccttiioonn))>>  ==  SSttddHHaasshhMMaapp::::nneeww(());;
                ///  RReessoollvvee  tthhee  bbeesstt  aavvaaiillaabbllee  ttrraaddeeaabbllee  mmaarrkkeettss  ffoorr  eeaacchh  ssttrraatteeggyy  llaannee..                lleett  mmaarrkkeett__ttaarrggeettss::  VVeecc<<((AAsssseett,,  TTiimmeeffrraammee))>>  ==  vveecc!![[                        ((AAsssseett::::BBTTCC,,  TTiimmeeffrraammee::::MMiinn1155)),,                        ((AAsssseett::::BBTTCC,,  TTiimmeeffrraammee::::HHoouurr11)),,                        ((AAsssseett::::EETTHH,,  TTiimmeeffrraammee::::MMiinn1155)),,                        ((AAsssseett::::EETTHH,,  TTiimmeeffrraammee::::HHoouurr11)),,                ]];;
                ffoorr  ((aasssseett,,  ttff))  iinn  &&mmaarrkkeett__ttaarrggeettss  {{
                        mmaattcchh  oorrddeerrbbooookk__ffeeeedd__cclliieenntt                                ..ffiinndd__ttrraaddeeaabbllee__mmaarrkkeett__ffoorr__ssiiggnnaall((**aasssseett,,  **ttff))                                ..aawwaaiitt                        {{
                                SSoommee((mmaarrkkeett))  ==>>  {{
                                        lleett  rreessoollvveedd__sslluugg  ==  mmaarrkkeett..sslluugg..cclloonnee(())..uunnwwrraapp__oorr__eellssee((||||  mmaarrkkeett..qquueessttiioonn..cclloonnee(())));;
                                        ///  EEaacchh  mmaarrkkeett  hhaass  ttwwoo  ttookkeennss::  YYEESS  aanndd  NNOO                                        ///  WWee  nneeeedd  ttoo  iiddeennttiiffyy  wwhhiicchh  iiss  UUPP  ((YYEESS))  aanndd  wwhhiicchh  iiss  DDOOWWNN  ((NNOO))                                        ffoorr  ((iiddxx,,  ttookkeenn))  iinn  mmaarrkkeett..ttookkeennss..iitteerr(())..eennuummeerraattee(())  {{
                                                ttookkeenn__iiddss..ppuusshh((ttookkeenn..ttookkeenn__iidd..cclloonnee(())));;
                                                ///  FFiirrsstt  ttookkeenn  iiss  ttyyppiiccaallllyy  YYEESS  ((UUPP)),,  sseeccoonndd  iiss  NNOO  ((DDOOWWNN))                                                lleett  oouuttccoommee  ==  ttookkeenn..oouuttccoommee..ttoo__aasscciiii__lloowweerrccaassee(());;
                                                lleett  ddiirreeccttiioonn  ==  iiff  oouuttccoommee..ccoonnttaaiinnss((""yyeess""))  ||||  oouuttccoommee..ccoonnttaaiinnss((""uupp""))  {{
                                                        DDiirreeccttiioonn::::UUpp                                                }}
  eellssee  iiff  oouuttccoommee..ccoonnttaaiinnss((""nnoo""))  ||||  oouuttccoommee..ccoonnttaaiinnss((""ddoowwnn""))  {{
                                                        DDiirreeccttiioonn::::DDoowwnn                                                }}
  eellssee  iiff  iiddxx  ====  00  {{
                                                        DDiirreeccttiioonn::::UUpp                                                }}
  eellssee  {{
                                                        DDiirreeccttiioonn::::DDoowwnn                                                }}
;;
                                                ttookkeenn__mmaapp..iinnsseerrtt((ttookkeenn..ttookkeenn__iidd..cclloonnee(()),,  ((**aasssseett,,  **ttff,,  ddiirreeccttiioonn))));;
                                                iinnffoo!!((                                                        mmaarrkkeett__sslluugg  ==  %%rreessoollvveedd__sslluugg,,                                                        ttookkeenn__iidd  ==  %%ttookkeenn..ttookkeenn__iidd,,                                                        aasssseett  ==  ??aasssseett,,                                                        ttiimmeeffrraammee  ==  ??ttff,,                                                        ddiirreeccttiioonn  ==  ??ddiirreeccttiioonn,,                                                        oouuttccoommee  ==  %%ttookkeenn..oouuttccoommee,,                                                        ""MMaappppeedd  mmaarrkkeett  ffoorr  oorrddeerrbbooookk  ffeeeedd""                                                ));;
                                        }}
                                }}
                                NNoonnee  ==>>  {{
                                        wwaarrnn!!((aasssseett  ==  ??aasssseett,,  ttiimmeeffrraammee  ==  ??ttff,,  ""NNoo  ttrraaddeeaabbllee  mmaarrkkeett  ffoouunndd  ffoorr  oorrddeerrbbooookk  ffeeeedd""));;
                                }}
                        }}
                }}
                iiff  ttookkeenn__iiddss..iiss__eemmppttyy(())  {{
                        wwaarrnn!!((""üüììää  NNoo  mmaarrkkeettss  ffoouunndd  ffoorr  oorrddeerrbbooookk  ffeeeedd,,  sskkiippppiinngg  WWeebbSSoocckkeett  ccoonnnneeccttiioonn""));;
                        rreettuurrnn;;
                }}
                iinnffoo!!((                        ccoouunntt  ==  ttookkeenn__iiddss..lleenn(()),,                        mmaappppeedd  ==  ttookkeenn__mmaapp..lleenn(()),,                        ""üüììää  SSttaarrttiinngg  PPoollyymmaarrkkeett  oorrddeerrbbooookk  WWeebbSSoocckkeett""                ));;
                lleett  ((eevveenntt__ttxx,,  mmuutt  eevveenntt__rrxx))  ==  mmppsscc::::cchhaannnneell::::<<WWssEEvveenntt>>((110000));;
                lleett  uurrll  ==  ""wwssss::///wwss--ssuubbssccrriippttiioonnss--cclloobb..ppoollyymmaarrkkeett..ccoomm//wwss//mmaarrkkeett"";;
                lleett  cclliieenntt  ==  MMaarrkkeettFFeeeeddCClliieenntt::::nneeww((uurrll,,  eevveenntt__ttxx));;
                lleett  ((ssuubbssccrriibbee__ttxx,,  ssuubbssccrriibbee__rrxx))  ==  mmppsscc::::cchhaannnneell::::<<VVeecc<<SSttrriinngg>>>>((1100));;
                lleett  ((sshhuuttddoowwnn__ttxx,,  sshhuuttddoowwnn__rrxx))  ==  mmppsscc::::cchhaannnneell::::<<(())>>((11));;
                ///  RRuunn  tthhee  cclliieenntt  iinn  aa  sseeppaarraattee  ttaasskk                lleett  cclliieenntt__hhaannddllee  ==  ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                        iiff  lleett  EErrrr((ee))  ==  cclliieenntt..rruunn((ttookkeenn__iiddss,,  ssuubbssccrriibbee__rrxx,,  sshhuuttddoowwnn__rrxx))..aawwaaiitt  {{
                                eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""OOrrddeerrbbooookk  ffeeeedd  cclliieenntt  eerrrroorr""));;
                        }}
                }}
));;
                ///  PPrroocceessss  eevveennttss  aanndd  uuppddaattee  ttrraacckkeerr  ‚‚ÄÄîî  uussiinngg  ttookkeenn__iidd  mmaappppiinngg  ffoorr  ccoorrrreecctt  rroouuttiinngg                wwhhiillee  lleett  SSoommee((eevveenntt))  ==  eevveenntt__rrxx..rreeccvv(())..aawwaaiitt  {{
                        mmaattcchh  eevveenntt  {{
                                WWssEEvveenntt::::BBooookkUUppddaattee((bbooookk))  ==>>  {{
                                        ///  CCaallccuullaattee  mmiiddppooiinntt  ffrroomm  oorrddeerrbbooookk  aass  tthhee  rreeaall  sshhaarree  pprriiccee                                        lleett  mmiiddppooiinntt  ==  bbooookk..mmiidd__pprriiccee(())..uunnwwrraapp__oorr((00..00));;
                                        lleett  sspprreeaadd  ==  bbooookk..sspprreeaadd(())..uunnwwrraapp__oorr((00..00));;
                                        lleett  sspprreeaadd__bbppss  ==  iiff  mmiiddppooiinntt  >>  00..00  {{
                                                sspprreeaadd  //  mmiiddppooiinntt  **  1100000000..00                                        }}
  eellssee  {{
                                                00..00                                        }}
;;
                                        ///  UUppddaattee  oorrddeerrbbooookk  ttrraacckkeerr                                        iiff  lleett  OOkk((mmuutt  ttrraacckkeerr))  ==  oorrddeerrbbooookk__ffeeeedd__ttrraacckkeerr..lloocckk(())  {{
                                                ///  RRoouuttee  ttoo  ccoorrrreecctt  ((AAsssseett,,  TTiimmeeffrraammee))  uussiinngg  ttookkeenn__iidd  mmaappppiinngg                                                iiff  lleett  SSoommee((&&((aasssseett,,  ttff,,  ddiirreeccttiioonn))))  ==  ttookkeenn__mmaapp..ggeett((&&bbooookk..ttookkeenn__iidd))  {{
                                                        ttrraacckkeerr..uuppddaattee__oorrddeerrbbooookk((&&bbooookk,,  aasssseett,,  ttff));;
                                                        ///  UUppddaattee  rreeaall  sshhaarree  pprriiccee  ffoorr  ppaappeerr  ttrraaddiinngg                                                        iiff  mmiiddppooiinntt  >>  00..00  {{
                                                                lleett  bbiidd  ==  bbooookk..bbeesstt__bbiidd(())..mmaapp((||bb||  bb..pprriiccee))..uunnwwrraapp__oorr((mmiiddppooiinntt));;
                                                                lleett  aasskk  ==  bbooookk..bbeesstt__aasskk(())..mmaapp((||aa||  aa..pprriiccee))..uunnwwrraapp__oorr((mmiiddppooiinntt));;
                                                                lleett  bbiidd__ssiizzee  ==  bbooookk..bbeesstt__bbiidd(())..mmaapp((||bb||  bb..ssiizzee))..uunnwwrraapp__oorr((00..00));;
                                                                lleett  aasskk__ssiizzee  ==  bbooookk..bbeesstt__aasskk(())..mmaapp((||aa||  aa..ssiizzee))..uunnwwrraapp__oorr((00..00));;
                                                                lleett  ddeepptthh__ttoopp55  ==                                                                        bbooookk..bbiiddss..iitteerr(())..ttaakkee((55))..mmaapp((||bb||  bb..ssiizzee))..ssuumm::::<<ff6644>>(())                                                                                ++  bbooookk..aasskkss..iitteerr(())..ttaakkee((55))..mmaapp((||aa||  aa..ssiizzee))..ssuumm::::<<ff6644>>(());;
                                                                lleett  ddiirreeccttiioonn__ssttrr  ==  mmaattcchh  ddiirreeccttiioonn  {{
                                                                        DDiirreeccttiioonn::::UUpp  ==>>  ""UUPP"",,                                                                        DDiirreeccttiioonn::::DDoowwnn  ==>>  ""DDOOWWNN"",,                                                                }}
;;
                                                                oorrddeerrbbooookk__sshhaarree__pprriicceess..uuppddaattee__qquuoottee__wwiitthh__ddeepptthh((                                                                        aasssseett,,                                                                        ttff,,                                                                        ddiirreeccttiioonn__ssttrr,,                                                                        bbiidd,,                                                                        aasskk,,                                                                        mmiiddppooiinntt,,                                                                        bbiidd__ssiizzee,,                                                                        aasskk__ssiizzee,,                                                                        ddeepptthh__ttoopp55,,                                                                ));;
                                                                iinnffoo!!((                                                                        ttookkeenn__iidd  ==  %%bbooookk..ttookkeenn__iidd,,                                                                        aasssseett  ==  ??aasssseett,,                                                                        ttiimmeeffrraammee  ==  ??ttff,,                                                                        ddiirreeccttiioonn  ==  ??ddiirreeccttiioonn,,                                                                        bbiidd  ==  bbiidd,,                                                                        aasskk  ==  aasskk,,                                                                        mmiidd  ==  mmiiddppooiinntt,,                                                                        sspprreeaadd__bbppss  ==  sspprreeaadd__bbppss,,                                                                        ""üüììää  RReeaall  sshhaarree  pprriicceess  uuppddaatteedd  ffrroomm  oorrddeerrbbooookk""                                                                ));;
                                                        }}
                                                }}
  eellssee  {{
                                                        ///  UUnnkknnoowwnn  ttookkeenn__iidd  ‚‚ÄÄîî  sskkiipp  ((ddoonn''tt  ppoolllluuttee  ootthheerr  aasssseettss))                                                        wwaarrnn!!((ttookkeenn__iidd  ==  %%bbooookk..ttookkeenn__iidd,,  ""üüììää  UUnnkknnoowwnn  ttookkeenn__iidd  iinn  bbooookk  uuppddaattee,,  sskkiippppiinngg""));;
                                                }}
                                        }}
                                }}
                                WWssEEvveenntt::::MMaarrkkeettUUppddaattee((ddaattaa))  ==>>  {{
                                        lleett  mmaappppeedd  ==  ttookkeenn__mmaapp..ggeett((&&ddaattaa..ttookkeenn__iidd))..ccooppiieedd(());;
                                        iiff  lleett  SSoommee((bbooookk))  ==  &&ddaattaa..oorrddeerrbbooookk  {{
                                                lleett  mmiiddppooiinntt  ==  bbooookk..mmiidd__pprriiccee(())..uunnwwrraapp__oorr((00..00));;
                                                iiff  lleett  OOkk((mmuutt  ttrraacckkeerr))  ==  oorrddeerrbbooookk__ffeeeedd__ttrraacckkeerr..lloocckk(())  {{
                                                        iiff  lleett  SSoommee((((aasssseett,,  ttff,,  ddiirreeccttiioonn))))  ==  mmaappppeedd  {{
                                                                ttrraacckkeerr..uuppddaattee__oorrddeerrbbooookk((bbooookk,,  aasssseett,,  ttff));;
                                                                ///  UUppddaattee  rreeaall  sshhaarree  pprriicceess                                                                iiff  mmiiddppooiinntt  >>  00..00  {{
                                                                        lleett  bbiidd  ==  bbooookk..bbeesstt__bbiidd(())..mmaapp((||bb||  bb..pprriiccee))..uunnwwrraapp__oorr((mmiiddppooiinntt));;
                                                                        lleett  aasskk  ==  bbooookk..bbeesstt__aasskk(())..mmaapp((||aa||  aa..pprriiccee))..uunnwwrraapp__oorr((mmiiddppooiinntt));;
                                                                        lleett  bbiidd__ssiizzee  ==  bbooookk..bbeesstt__bbiidd(())..mmaapp((||bb||  bb..ssiizzee))..uunnwwrraapp__oorr((00..00));;
                                                                        lleett  aasskk__ssiizzee  ==  bbooookk..bbeesstt__aasskk(())..mmaapp((||aa||  aa..ssiizzee))..uunnwwrraapp__oorr((00..00));;
                                                                        lleett  ddeepptthh__ttoopp55  ==                                                                                bbooookk..bbiiddss..iitteerr(())..ttaakkee((55))..mmaapp((||bb||  bb..ssiizzee))..ssuumm::::<<ff6644>>(())                                                                                        ++  bbooookk..aasskkss..iitteerr(())..ttaakkee((55))..mmaapp((||aa||  aa..ssiizzee))..ssuumm::::<<ff6644>>(());;
                                                                        lleett  ddiirreeccttiioonn__ssttrr  ==  mmaattcchh  ddiirreeccttiioonn  {{
                                                                                DDiirreeccttiioonn::::UUpp  ==>>  ""UUPP"",,                                                                                DDiirreeccttiioonn::::DDoowwnn  ==>>  ""DDOOWWNN"",,                                                                        }}
;;
                                                                        oorrddeerrbbooookk__sshhaarree__pprriicceess..uuppddaattee__qquuoottee__wwiitthh__ddeepptthh((                                                                                aasssseett,,                                                                                ttff,,                                                                                ddiirreeccttiioonn__ssttrr,,                                                                                bbiidd,,                                                                                aasskk,,                                                                                mmiiddppooiinntt,,                                                                                bbiidd__ssiizzee,,                                                                                aasskk__ssiizzee,,                                                                                ddeepptthh__ttoopp55,,                                                                        ));;
                                                                }}
                                                        }}
                                                }}
                                        }}
                                        iiff  lleett  SSoommee((ttrraaddee__ssiiddee))  ==  ddaattaa..llaasstt__ttrraaddee__ssiiddee  {{
                                                lleett  ttrraaddee  ==  ccrraattee::::cclloobb::::ttyyppeess::::TTrraaddee  {{
                                                        iidd::  ffoorrmmaatt!!((""ttrraaddee--{{
}}
"",,  ddaattaa..ttiimmeessttaammpp)),,                                                        oorrddeerr__iidd::  eetthheerrss::::ttyyppeess::::HH225566::::zzeerroo(()),,                                                        ttookkeenn__iidd::  ddaattaa..ttookkeenn__iidd..cclloonnee(()),,                                                        ssiiddee::  ttrraaddee__ssiiddee,,                                                        pprriiccee::  ddaattaa..llaasstt__pprriiccee..uunnwwrraapp__oorr((00..00)),,                                                        ssiizzee::  00..00,,                                                        ttiimmeessttaammpp::  ddaattaa..ttiimmeessttaammpp,,                                                        ttaakkeerr::  eetthheerrss::::ttyyppeess::::AAddddrreessss::::zzeerroo(()),,                                                        mmaakkeerr::  eetthheerrss::::ttyyppeess::::AAddddrreessss::::zzeerroo(()),,                                                }}
;;
                                                iiff  lleett  OOkk((mmuutt  ttrraacckkeerr))  ==  oorrddeerrbbooookk__ffeeeedd__ttrraacckkeerr..lloocckk(())  {{
                                                        iiff  lleett  SSoommee((((aasssseett,,  ttff,,  __))))  ==  mmaappppeedd  {{
                                                                ttrraacckkeerr..pprroocceessss__ttrraaddee((&&ttrraaddee,,  aasssseett,,  ttff));;
                                                        }}
                                                }}
                                        }}
                                }}
                                WWssEEvveenntt::::CCoonnnneecctteedd  ==>>  {{
                                        iinnffoo!!((""üüììää  PPoollyymmaarrkkeett  oorrddeerrbbooookk  WWeebbSSoocckkeett  ccoonnnneecctteedd""));;
                                }}
                                WWssEEvveenntt::::DDiissccoonnnneecctteedd  ==>>  {{
                                        wwaarrnn!!((""üüììää  PPoollyymmaarrkkeett  oorrddeerrbbooookk  WWeebbSSoocckkeett  ddiissccoonnnneecctteedd""));;
                                }}
                                WWssEEvveenntt::::EErrrroorr((ee))  ==>>  {{
                                        eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""üüììää  PPoollyymmaarrkkeett  oorrddeerrbbooookk  WWeebbSSoocckkeett  eerrrroorr""));;
                                }}
                                __  ==>>  {{
}}
                        }}
                }}
                lleett  __  ==  sshhuuttddoowwnn__ttxx..sseenndd(((())))..aawwaaiitt;;
                lleett  __  ==  cclliieenntt__hhaannddllee..aawwaaiitt;;
        }}
));;
        ///  SSppaawwnn  oorraaccllee  ssoouurrcceess  ((nnaattiivvee  mmooddee  ==  RRTTDDSS  oonnllyy,,  ootthheerrwwiissee  RRTTDDSS  ++  ooppttiioonnaall  BBiinnaannccee))..        lleett  oorraaccllee__eevveenntt__ttxx  ==  pprriiccee__ttxx..cclloonnee(());;
        lleett  oorraaccllee__ppaappeerr__ttxx  ==  ppaappeerr__pprriiccee__ttxx..cclloonnee(());;
        lleett  oorraaccllee__ppaappeerr__eennaabblleedd  ==  ppaappeerr__ttrraaddiinngg__eennaabblleedd;;
        lleett  oorraaccllee__aasssseettss  ==  ccoonnffiigg..bboott..aasssseettss..cclloonnee(());;
        lleett  oorraaccllee__ppeerrssiisstteennccee  ==  ccssvv__ppeerrssiisstteennccee..cclloonnee(());;
        lleett  oorraaccllee__ccffgg  ==  ccoonnffiigg..oorraaccllee..cclloonnee(());;
        lleett  ppoollyymmaarrkkeett__ccffgg  ==  ccoonnffiigg..ppoollyymmaarrkkeett..cclloonnee(());;
        lleett  oorraaccllee__hhaannddllee  ==  ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                uussee  ccrraattee::::oorraaccllee::::ssoouurrcceess::::{{
BBiinnaanncceeCClliieenntt,,  PPrriicceeSSoouurrccee  aass  __,,  RRttddssCClliieenntt,,  SSoouurrcceeEEvveenntt}}
;;
                uussee  ccrraattee::::ppeerrssiisstteennccee::::PPrriicceeRReeccoorrdd;;
                lleett  aasssseettss::  VVeecc<<AAsssseett>>  ==  oorraaccllee__aasssseettss                        ..iitteerr(())                        ..ffiilltteerr__mmaapp((||ss||  AAsssseett::::ffrroomm__ssttrr((ss))))                        ..ccoolllleecctt(());;
                lleett  nnaattiivvee__oonnllyy  ==  ppoollyymmaarrkkeett__ccffgg..ddaattaa__mmooddee..eeqq__iiggnnoorree__aasscciiii__ccaassee((""nnaattiivvee__oonnllyy""));;
                lleett  eennaabbllee__rrttddss  ==  oorraaccllee__ccffgg..rrttddss__eennaabblleedd  &&&&  ppoollyymmaarrkkeett__ccffgg..rrttddss..eennaabblleedd;;
                lleett  eennaabbllee__bbiinnaannccee  ==  oorraaccllee__ccffgg..bbiinnaannccee__eennaabblleedd  &&&&  !!nnaattiivvee__oonnllyy;;
                ///  CCrreeaattee  eevveenntt  cchhaannnneell  ffoorr  oorraaccllee  ((sshhaarreedd  bbeettwweeeenn  BBiinnaannccee  aanndd  RRTTDDSS))                lleett  ((eevveenntt__ttxx,,  mmuutt  eevveenntt__rrxx))  ==  mmppsscc::::cchhaannnneell::::<<SSoouurrcceeEEvveenntt>>((11000000));;
                iiff  !!eennaabbllee__bbiinnaannccee  &&&&  !!eennaabbllee__rrttddss  {{
                        ttrraacciinngg::::wwaarrnn!!((                                ""NNoo  oorraaccllee  ssoouurrccee  eennaabblleedd;;
  sseett  oorraaccllee..rrttddss__eennaabblleedd==ttrruuee  ffoorr  nnaattiivvee  mmooddee""                        ));;
                        rreettuurrnn;;
                }}
                ///  SSppaawwnn  BBiinnaannccee  ccoonnnneeccttiioonn  oonnllyy  wwhheenn  nnaattiivvee__oonnllyy  iiss  ddiissaabblleedd..                lleett  bbiinnaannccee__hhaannddllee  ==  iiff  eennaabbllee__bbiinnaannccee  {{
                        lleett  bbiinnaannccee__ttxx  ==  eevveenntt__ttxx..cclloonnee(());;
                        lleett  bbiinnaannccee__aasssseettss  ==  aasssseettss..cclloonnee(());;
                        SSoommee((ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                                lleett  mmuutt  cclliieenntt  ==  BBiinnaanncceeCClliieenntt::::nneeww(());;
                                iiff  lleett  EErrrr((ee))  ==  cclliieenntt..ssuubbssccrriibbee((&&bbiinnaannccee__aasssseettss))..aawwaaiitt  {{
                                        ttrraacciinngg::::eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""BBiinnaannccee  ssuubbssccrriibbee  ffaaiilleedd""));;
                                        rreettuurrnn;;
                                }}
                                iiff  lleett  EErrrr((ee))  ==  cclliieenntt..ccoonnnneecctt((bbiinnaannccee__ttxx))..aawwaaiitt  {{
                                        ttrraacciinngg::::eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""BBiinnaannccee  ccoonnnneeccttiioonn  ffaaiilleedd""));;
                                }}
                        }}
))))                }}
  eellssee  {{
                        ttrraacciinngg::::iinnffoo!!((""BBiinnaannccee  ffeeeedd  ddiissaabblleedd  bbyy  ppoollyymmaarrkkeett..ddaattaa__mmooddee==nnaattiivvee__oonnllyy""));;
                        NNoonnee                }}
;;
                ///  SSppaawwnn  RRTTDDSS  ccoonnnneeccttiioonn  ((PPoollyymmaarrkkeett  ffeeeedd))..                lleett  rrttddss__hhaannddllee  ==  iiff  eennaabbllee__rrttddss  {{
                        lleett  rrttddss__ttxx  ==  eevveenntt__ttxx..cclloonnee(());;
                        lleett  rrttddss__aasssseettss  ==  aasssseettss..cclloonnee(());;
                        SSoommee((ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                                lleett  mmuutt  cclliieenntt  ==  RRttddssCClliieenntt::::nneeww(());;
                                iiff  lleett  EErrrr((ee))  ==  cclliieenntt..ssuubbssccrriibbee((&&rrttddss__aasssseettss))..aawwaaiitt  {{
                                        ttrraacciinngg::::eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""RRTTDDSS  ssuubbssccrriibbee  ffaaiilleedd""));;
                                        rreettuurrnn;;
                                }}
                                iiff  lleett  EErrrr((ee))  ==  cclliieenntt..ccoonnnneecctt((rrttddss__ttxx))..aawwaaiitt  {{
                                        ttrraacciinngg::::eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""RRTTDDSS  ccoonnnneeccttiioonn  ffaaiilleedd""));;
                                }}
                        }}
))))                }}
  eellssee  {{
                        ttrraacciinngg::::wwaarrnn!!((""RRTTDDSS  ffeeeedd  ddiissaabblleedd;;
  nnaattiivvee  PPoollyymmaarrkkeett  ddaattaa  wwiillll  bbee  uunnaavvaaiillaabbllee""));;
                        NNoonnee                }}
;;
                lleett  mmuutt  ttiicckk__ccoouunntt::  uu6644  ==  00;;
                ///  PPrroocceessss  eevveennttss  aanndd  ccoonnvveerrtt  ttoo  PPrriicceeTTiicckkss                wwhhiillee  lleett  SSoommee((eevveenntt))  ==  eevveenntt__rrxx..rreeccvv(())..aawwaaiitt  {{
                        mmaattcchh  eevveenntt  {{
                                SSoouurrcceeEEvveenntt::::TTiicckk((ttiicckk))  ==>>  {{
                                        ttiicckk__ccoouunntt  ++==  11;;
                                        iiff  ttiicckk__ccoouunntt  %%  110000  ====  00  {{
                                                ttrraacciinngg::::iinnffoo!!((                                                        ccoouunntt  ==  ttiicckk__ccoouunntt,,                                                        aasssseett  ==  ??ttiicckk..aasssseett,,                                                        ssoouurrccee  ==  ??ttiicckk..ssoouurrccee,,                                                        ""üüììàà  RReecceeiivveedd  pprriiccee  ttiicckk""                                                ));;
                                        }}
                                        lleett  pprriiccee__ttiicckk  ==  PPrriicceeTTiicckk  {{
                                                eexxcchhaannggee__ttss::  ttiicckk..ttss,,                                                llooccaall__ttss::  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp__mmiilllliiss(()),,                                                aasssseett::  ttiicckk..aasssseett,,                                                bbiidd::  ttiicckk..bbiidd,,                                                aasskk::  ttiicckk..aasskk,,                                                mmiidd::  ttiicckk..mmiidd,,                                                ssoouurrccee::  ttiicckk..ssoouurrccee,,                                                llaatteennccyy__mmss::  ttiicckk..llaatteennccyy__mmss  aass  uu6644,,                                        }}
;;
                                        ///  FFoorrwwaarrdd  ttoo  ppaappeerr  ttrraaddiinngg  eennggiinnee  iiff  eennaabblleedd                                        iiff  oorraaccllee__ppaappeerr__eennaabblleedd  {{
                                                lleett  __  ==  oorraaccllee__ppaappeerr__ttxx..sseenndd((pprriiccee__ttiicckk..cclloonnee(())))..aawwaaiitt;;
                                        }}
                                        iiff  lleett  EErrrr((ee))  ==  oorraaccllee__eevveenntt__ttxx..sseenndd((pprriiccee__ttiicckk))..aawwaaiitt  {{
                                                ttrraacciinngg::::eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  sseenndd  pprriiccee  ttiicckk""));;
                                        }}
                                        ///  SSaavvee  pprriiccee  ttoo  CCSSVV  eevveerryy  110000  ttiicckkss  ((ssaammpplliinngg))                                        iiff  ttiicckk__ccoouunntt  %%  110000  ====  00  {{
                                                lleett  rreeccoorrdd  ==  PPrriicceeRReeccoorrdd  {{
                                                        ttiimmeessttaammpp::  ttiicckk..ttss,,                                                        aasssseett::  ffoorrmmaatt!!((""{{
::??}}
"",,  ttiicckk..aasssseett)),,                                                        pprriiccee::  ttiicckk..mmiidd,,                                                        ssoouurrccee::  ffoorrmmaatt!!((""{{
::??}}
"",,  ttiicckk..ssoouurrccee)),,                                                        vvoolluummee::  NNoonnee,,                                                }}
;;
                                                iiff  lleett  EErrrr((ee))  ==  oorraaccllee__ppeerrssiisstteennccee..ssaavvee__pprriiccee((rreeccoorrdd))..aawwaaiitt  {{
                                                        ttrraacciinngg::::wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  ssaavvee  pprriiccee  ttoo  CCSSVV""));;
                                                }}
                                        }}
                                }}
                                SSoouurrcceeEEvveenntt::::CCoonnnneecctteedd((ssoouurrccee))  ==>>  {{
                                        ttrraacciinngg::::iinnffoo!!((ssoouurrccee  ==  %%ssoouurrccee,,  ""üüîîåå  OOrraaccllee  ssoouurrccee  ccoonnnneecctteedd""));;
                                }}
                                SSoouurrcceeEEvveenntt::::DDiissccoonnnneecctteedd((ssoouurrccee))  ==>>  {{
                                        ttrraacciinngg::::wwaarrnn!!((ssoouurrccee  ==  %%ssoouurrccee,,  ""üüîîåå  OOrraaccllee  ssoouurrccee  ddiissccoonnnneecctteedd""));;
                                }}
                                SSoouurrcceeEEvveenntt::::CCoommmmeenntt((ccoommmmeenntt))  ==>>  {{
                                        ttrraacciinngg::::ddeebbuugg!!((                                                ttooppiicc  ==  %%ccoommmmeenntt..ttooppiicc,,                                                uusseerrnnaammee  ==  ??ccoommmmeenntt..uusseerrnnaammee,,                                                ssyymmbbooll  ==  ??ccoommmmeenntt..ssyymmbbooll,,                                                bbooddyy  ==  ??ccoommmmeenntt..bbooddyy,,                                                ""RRTTDDSS  ccoommmmeenntt  eevveenntt  rreecceeiivveedd""                                        ));;
                                }}
                                SSoouurrcceeEEvveenntt::::EErrrroorr((ssoouurrccee,,  eerrrr))  ==>>  {{
                                        ttrraacciinngg::::eerrrroorr!!((ssoouurrccee  ==  %%ssoouurrccee,,  eerrrroorr  ==  %%eerrrr,,  ""OOrraaccllee  eerrrroorr""));;
                                }}
                                __  ==>>  {{
}}
                        }}
                }}
                iiff  lleett  SSoommee((hhaannddllee))  ==  bbiinnaannccee__hhaannddllee  {{
                        hhaannddllee..aabboorrtt(());;
                }}
                iiff  lleett  SSoommee((hhaannddllee))  ==  rrttddss__hhaannddllee  {{
                        hhaannddllee..aabboorrtt(());;
                }}
        }}
));;
        ///  FFeeaattuurree  eennggiinnee  ttaasskk  --  pprroocceesssseess  pprriiccee  ttiicckkss  aanndd  ggeenneerraatteess  ffeeaattuurreess        lleett  ffeeaattuurree__eennggiinnee__iinnnneerr  ==  ffeeaattuurree__eennggiinnee..cclloonnee(());;
        lleett  ffeeaattuurree__nnaattiivvee__oonnllyy  ==  ccoonnffiigg                ..ppoollyymmaarrkkeett                ..ddaattaa__mmooddee                ..eeqq__iiggnnoorree__aasscciiii__ccaassee((""nnaattiivvee__oonnllyy""));;
        lleett  ffeeaattuurree__cclloobb__cclliieenntt  ==  cclloobb__cclliieenntt..cclloonnee(());;
        lleett  ffeeaattuurree__ddaattaa__ddiirr  ==  ccoonnffiigg..ppeerrssiisstteennccee..ddaattaa__ddiirr..cclloonnee(());;
        lleett  ffeeaattuurree__hhaannddllee  ==  ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                uussee  ccrraattee::::cclloobb::::PPrriicceeHHiissttoorryyIInntteerrvvaall;;
                uussee  ccrraattee::::oorraaccllee::::ssoouurrcceess::::BBiinnaanncceeCClliieenntt;;
                uussee  ccrraattee::::oorraaccllee::::CCaannddlleeBBuuiillddeerr;;
                uussee  ssttdd::::ccoolllleeccttiioonnss::::HHaasshhMMaapp;;
                lleett  mmuutt  ccaannddllee__bbuuiillddeerr  ==  CCaannddlleeBBuuiillddeerr::::nneeww((550000));;
                lleett  mmuutt  llaasstt__ffeeaattuurree__ttiimmee::  HHaasshhMMaapp<<((AAsssseett,,  TTiimmeeffrraammee)),,  ii6644>>  ==  HHaasshhMMaapp::::nneeww(());;
                lleett  mmuutt  ttiicckk__ccoouunntt::  uu6644  ==  00;;
                ///  FFeettcchh  hhiissttoorriiccaall  ccaannddlleess  aatt  ssttaarrttuupp  oonnllyy  wwhheenn  nnaattiivvee--oonnllyy  mmooddee  iiss  ddiissaabblleedd..                iiff  !!ffeeaattuurree__nnaattiivvee__oonnllyy  {{
                        ttrraacciinngg::::iinnffoo!!((""FFeettcchhiinngg  hhiissttoorriiccaall  ccaannddlleess  ffrroomm  BBiinnaannccee......""));;
                        ffoorr  aasssseett  iinn  [[AAsssseett::::BBTTCC,,  AAsssseett::::EETTHH]]  {{
                                ffoorr  ttiimmeeffrraammee  iinn  [[TTiimmeeffrraammee::::MMiinn1155,,  TTiimmeeffrraammee::::HHoouurr11]]  {{
                                        mmaattcchh  BBiinnaanncceeCClliieenntt::::ffeettcchh__hhiissttoorriiccaall__ccaannddlleess((aasssseett,,  ttiimmeeffrraammee,,  110000))..aawwaaiitt  {{
                                                OOkk((ccaannddlleess))  ==>>  {{
                                                        lleett  ccoouunntt  ==  ccaannddlleess..lleenn(());;
                                                        ccaannddllee__bbuuiillddeerr..sseeeedd__hhiissttoorryy((ccaannddlleess));;
                                                        ttrraacciinngg::::iinnffoo!!((                                                                aasssseett  ==  ??aasssseett,,                                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                                ccaannddllee__ccoouunntt  ==  ccoouunntt,,                                                                ""SSeeeeddeedd  hhiissttoorriiccaall  ccaannddlleess""                                                        ));;
                                                }}
                                                EErrrr((ee))  ==>>  {{
                                                        ttrraacciinngg::::wwaarrnn!!((                                                                aasssseett  ==  ??aasssseett,,                                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                                eerrrroorr  ==  %%ee,,                                                                ""FFaaiilleedd  ttoo  ffeettcchh  hhiissttoorriiccaall  ccaannddlleess,,  wwiillll  bbuuiilldd  ffrroomm  lliivvee  ddaattaa""                                                        ));;
                                                }}
                                        }}
                                }}
                        }}
                        ttrraacciinngg::::iinnffoo!!((""HHiissttoorriiccaall  ccaannddllee  ffeettcchh  ccoommpplleettee""));;
                }}
  eellssee  {{
                        ttrraacciinngg::::iinnffoo!!((""NNaattiivvee--oonnllyy  mmooddee::  bboooottssttrraappppiinngg  ffrroomm  PPoollyymmaarrkkeett  hhiissttoorriiccaall  ddaattaa""));;
                        iiff  lleett  EErrrr((ee))  ==  ffeeaattuurree__cclloobb__cclliieenntt..rreeffrreesshh__mmaarrkkeettss(())..aawwaaiitt  {{
                                ttrraacciinngg::::wwaarrnn!!((                                        eerrrroorr  ==  %%ee,,                                        ""FFaaiilleedd  ttoo  rreeffrreesshh  mmaarrkkeettss  bbeeffoorree  PPoollyymmaarrkkeett  wwaarrmmuupp  bboooottssttrraapp""                                ));;
                        }}
                        lleett  nnaattiivvee__ttaarrggeettss::  [[((&&ssttrr,,  AAsssseett,,  TTiimmeeffrraammee));;
  44]]  ==  [[                                ((""bbttcc--1155mm"",,  AAsssseett::::BBTTCC,,  TTiimmeeffrraammee::::MMiinn1155)),,                                ((""bbttcc--11hh"",,  AAsssseett::::BBTTCC,,  TTiimmeeffrraammee::::HHoouurr11)),,                                ((""eetthh--1155mm"",,  AAsssseett::::EETTHH,,  TTiimmeeffrraammee::::MMiinn1155)),,                                ((""eetthh--11hh"",,  AAsssseett::::EETTHH,,  TTiimmeeffrraammee::::HHoouurr11)),,                        ]];;
                        ffoorr  ((sslluugg,,  aasssseett,,  ttiimmeeffrraammee))  iinn  nnaattiivvee__ttaarrggeettss  {{
                                lleett  llooccaall__ppooiinnttss  ==  llooaadd__llooccaall__pprriiccee__ppooiinnttss((&&ffeeaattuurree__ddaattaa__ddiirr,,  aasssseett,,  9966));;
                                lleett  mmuutt  mmeerrggeedd  ==  bbuuiilldd__ccaannddlleess__ffrroomm__ppooiinnttss((aasssseett,,  ttiimmeeffrraammee,,  &&llooccaall__ppooiinnttss));;
                                lleett  llooccaall__ccoouunntt  ==  mmeerrggeedd..lleenn(());;
                                lleett  aanncchhoorr  ==  llooccaall__ppooiinnttss                                        ..llaasstt(())                                        ..mmaapp((||((__,,  pprriiccee))||  **pprriiccee))                                        ..uunnwwrraapp__oorr__eellssee((||||  ddeeffaauulltt__aanncchhoorr__pprriiccee((aasssseett))));;
                                iiff  llooccaall__ccoouunntt  <<  3300  {{
                                        mmaattcchh  bboooottssttrraapp__ppoollyymmaarrkkeett__hhiissttoorryy__ccaannddlleess((                                                ffeeaattuurree__cclloobb__cclliieenntt..aass__rreeff(()),,                                                sslluugg,,                                                aasssseett,,                                                ttiimmeeffrraammee,,                                                aanncchhoorr,,                                                mmaattcchh  ttiimmeeffrraammee  {{
                                                        TTiimmeeffrraammee::::MMiinn1155  ==>>  PPrriicceeHHiissttoorryyIInntteerrvvaall::::OOnneeDDaayy,,                                                        TTiimmeeffrraammee::::HHoouurr11  ==>>  PPrriicceeHHiissttoorryyIInntteerrvvaall::::OOnneeWWeeeekk,,                                                }}
,,                                        ))                                        ..aawwaaiitt                                        {{
                                                OOkk((mmuutt  ccaannddlleess))  ==>>  {{
                                                        lleett  rreemmoottee__ccoouunntt  ==  ccaannddlleess..lleenn(());;
                                                        iiff  rreemmoottee__ccoouunntt  >>  00  {{
                                                                mmeerrggeedd..aappppeenndd((&&mmuutt  ccaannddlleess));;
                                                                mmeerrggeedd  ==  ddeedduupp__ccaannddlleess__bbyy__ooppeenn__ttiimmee((mmeerrggeedd));;
                                                        }}
                                                        ttrraacciinngg::::iinnffoo!!((                                                                mmaarrkkeett__sslluugg  ==  sslluugg,,                                                                aasssseett  ==  ??aasssseett,,                                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                                llooccaall__ccaannddlleess  ==  llooccaall__ccoouunntt,,                                                                ppoollyymmaarrkkeett__ccaannddlleess  ==  rreemmoottee__ccoouunntt,,                                                                ttoottaall__ccaannddlleess  ==  mmeerrggeedd..lleenn(()),,                                                                ""NNaattiivvee  wwaarrmmuupp  ccaannddlleess  pprreeppaarreedd""                                                        ));;
                                                }}
                                                EErrrr((ee))  ==>>  {{
                                                        ttrraacciinngg::::wwaarrnn!!((                                                                mmaarrkkeett__sslluugg  ==  sslluugg,,                                                                aasssseett  ==  ??aasssseett,,                                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                                llooccaall__ccaannddlleess  ==  llooccaall__ccoouunntt,,                                                                eerrrroorr  ==  %%ee,,                                                                ""FFaaiilleedd  ttoo  bboooottssttrraapp  PPoollyymmaarrkkeett  hhiissttoorryy  ffoorr  nnaattiivvee  wwaarrmmuupp""                                                        ));;
                                                }}
                                        }}
                                }}
  eellssee  {{
                                        ttrraacciinngg::::iinnffoo!!((                                                mmaarrkkeett__sslluugg  ==  sslluugg,,                                                aasssseett  ==  ??aasssseett,,                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                llooccaall__ccaannddlleess  ==  llooccaall__ccoouunntt,,                                                ""NNaattiivvee  wwaarrmmuupp  ssaattiissffiieedd  ffrroomm  llooccaall  RRTTDDSS  hhiissttoorryy""                                        ));;
                                }}
                                iiff  !!mmeerrggeedd..iiss__eemmppttyy(())  {{
                                        lleett  kkeeeepp  ==  220000uussiizzee;;
                                        lleett  mmuutt  sseeeeddeedd  ==  mmeerrggeedd;;
                                        iiff  sseeeeddeedd..lleenn(())  >>  kkeeeepp  {{
                                                sseeeeddeedd  ==  sseeeeddeedd..sspplliitt__ooffff((sseeeeddeedd..lleenn(())  --  kkeeeepp));;
                                        }}
                                        lleett  sseeeeddeedd__ccoouunntt  ==  sseeeeddeedd..lleenn(());;
                                        ccaannddllee__bbuuiillddeerr..sseeeedd__hhiissttoorryy((sseeeeddeedd));;
                                        ttrraacciinngg::::iinnffoo!!((                                                mmaarrkkeett__sslluugg  ==  sslluugg,,                                                aasssseett  ==  ??aasssseett,,                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                sseeeeddeedd__ccaannddlleess  ==  sseeeeddeedd__ccoouunntt,,                                                ""SSeeeeddeedd  nnaattiivvee  wwaarrmmuupp  ccaannddlleess""                                        ));;
                                }}
  eellssee  {{
                                        ttrraacciinngg::::wwaarrnn!!((                                                mmaarrkkeett__sslluugg  ==  sslluugg,,                                                aasssseett  ==  ??aasssseett,,                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                ""NNoo  hhiissttoorriiccaall  ccaannddlleess  aavvaaiillaabbllee  ffoorr  nnaattiivvee  wwaarrmmuupp""                                        ));;
                                }}
                        }}
                }}
                ///  WWaarrmmuupp  iinnddiiccaattoorrss  ffrroomm  sseeeeddeedd  ccaannddlleess  wwhheenn  aavvaaiillaabbllee..                ttrraacciinngg::::iinnffoo!!((""WWaarrmmiinngg  uupp  FFeeaattuurreeEEnnggiinnee  ffrroomm  hhiissttoorriiccaall  ccaannddlleess......""));;
                ffoorr  aasssseett  iinn  [[AAsssseett::::BBTTCC,,  AAsssseett::::EETTHH]]  {{
                        ffoorr  ttiimmeeffrraammee  iinn  [[TTiimmeeffrraammee::::MMiinn1155,,  TTiimmeeffrraammee::::HHoouurr11]]  {{
                                lleett  ccaannddlleess  ==  ccaannddllee__bbuuiillddeerr..ggeett__llaasstt__nn((aasssseett,,  ttiimmeeffrraammee,,  110000));;
                                lleett  nn  ==  ccaannddlleess..lleenn(());;
                                iiff  nn  <<  3300  {{
                                        ttrraacciinngg::::wwaarrnn!!((                                                aasssseett  ==  ??aasssseett,,                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                ccaannddllee__ccoouunntt  ==  nn,,                                                ""NNoott  eennoouugghh  hhiissttoorriiccaall  ccaannddlleess  ffoorr  wwaarrmmuupp  ((nneeeedd  3300++))""                                        ));;
                                        ccoonnttiinnuuee;;
                                }}
                                ///  PPrrooggrreessssiivveellyy  ffeeeedd  iinnccrreeaassiinngg  sslliicceess  ttoo  bbuuiilldd  ssttaatteeffuull  iinnddiiccaattoorrss..                                lleett  mmuutt  ffee  ==  ffeeaattuurree__eennggiinnee__iinnnneerr..lloocckk(())..aawwaaiitt;;
                                lleett  ssttaarrtt  ==  1155..mmiinn((nn));;
                                ffoorr  eenndd  iinn  ssttaarrtt....==nn  {{
                                        lleett  sslliiccee  ==  &&ccaannddlleess[[....eenndd]];;
                                        ffee..ccoommppuuttee((sslliiccee));;
                                }}
                                ddrroopp((ffee));;
                                ttrraacciinngg::::iinnffoo!!((                                        aasssseett  ==  ??aasssseett,,                                        ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                        wwaarrmmuupp__sstteeppss  ==  nn  --  ssttaarrtt  ++  11,,                                        ""FFeeaattuurreeEEnnggiinnee  wwaarrmmeedd  uupp  (({{
}}
  ccaannddlleess  rreeppllaayyeedd))"",,                                        nn                                ));;
                        }}
                }}
                ttrraacciinngg::::iinnffoo!!((""FFeeaattuurreeEEnnggiinnee  wwaarrmmuupp  ccoommpplleettee""));;
                wwhhiillee  lleett  SSoommee((ttiicckk))  ==  pprriiccee__rrxx..rreeccvv(())..aawwaaiitt  {{
                        ttiicckk__ccoouunntt  ++==  11;;
                        iiff  ttiicckk__ccoouunntt  %%  110000  ====  00  {{
                                ttrraacciinngg::::ddeebbuugg!!((ccoouunntt  ==  ttiicckk__ccoouunntt,,  aasssseett  ==  ??ttiicckk..aasssseett,,  ""üüîîßß  FFeeaattuurree  ttaasskk  rreecceeiivveedd  ttiicckkss""));;
                        }}
                        ///  CCoonnvveerrtt  PPrriicceeTTiicckk  ttoo  NNoorrmmaalliizzeeddTTiicckk  ffoorr  ccaannddllee  bbuuiillddeerr                        lleett  nnoorrmmaalliizzeedd  ==  ccrraattee::::oorraaccllee::::NNoorrmmaalliizzeeddTTiicckk  {{
                                ttss::  ttiicckk..eexxcchhaannggee__ttss,,                                aasssseett::  ttiicckk..aasssseett,,                                bbiidd::  ttiicckk..bbiidd,,                                aasskk::  ttiicckk..aasskk,,                                mmiidd::  ttiicckk..mmiidd,,                                ssoouurrccee::  ttiicckk..ssoouurrccee,,                                llaatteennccyy__mmss::  ttiicckk..llaatteennccyy__mmss  aass  uu6644,,                        }}
;;
                        ///  PPrroocceessss  eeaacchh  ttiimmeeffrraammee                        ffoorr  ttiimmeeffrraammee  iinn  [[TTiimmeeffrraammee::::MMiinn1155,,  TTiimmeeffrraammee::::HHoouurr11]]  {{
                                ///  AAdddd  ttiicckk  ttoo  ccaannddllee  bbuuiillddeerr  ffoorr  tthhiiss  ttiimmeeffrraammee                                ccaannddllee__bbuuiillddeerr..aadddd__ttiicckk((&&nnoorrmmaalliizzeedd,,  ttiimmeeffrraammee));;
                                ///  GGeett  ccaannddlleess  ((rreettuurrnnss  VVeecc<<CCaannddllee>>,,  nnoott  OOppttiioonn))                                lleett  ccaannddlleess  ==  ccaannddllee__bbuuiillddeerr..ggeett__llaasstt__nn((ttiicckk..aasssseett,,  ttiimmeeffrraammee,,  5500));;
                                lleett  ccaannddllee__ccoouunntt  ==  ccaannddlleess..lleenn(());;
                                ///  LLoogg  ccaannddllee  ccoouunntt  ppeerriiooddiiccaallllyy  ((eevveerryy  1100  sseeccoonnddss  ppeerr  aasssseett//ttiimmeeffrraammee))                                lleett  nnooww  ==  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp__mmiilllliiss(());;
                                lleett  kkeeyy  ==  ((ttiicckk..aasssseett,,  ttiimmeeffrraammee));;
                                lleett  llaasstt  ==  llaasstt__ffeeaattuurree__ttiimmee..eennttrryy((kkeeyy))..oorr__iinnsseerrtt((00));;
                                iiff  nnooww  --  **llaasstt  >>  1100000000  {{
                                        **llaasstt  ==  nnooww;;
                                        ttrraacciinngg::::iinnffoo!!((                                                aasssseett  ==  ??ttiicckk..aasssseett,,                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                ccaannddllee__ccoouunntt  ==  ccaannddllee__ccoouunntt,,                                                ""üüïïØØÔÔ∏∏èè  CCaannddllee  ccoouunntt""                                        ));;
                                }}
                                ///  NNeeeedd  aatt  lleeaasstt  3300  ccaannddlleess  ffoorr  mmeeaanniinnggffuull  tteecchhnniiccaall  iinnddiiccaattoorrss                                iiff  ccaannddllee__ccoouunntt  >>==  3300  {{
                                        iiff  lleett  SSoommee((ffeeaattuurreess))  ==  ffeeaattuurree__eennggiinnee__iinnnneerr..lloocckk(())..aawwaaiitt..ccoommppuuttee((&&ccaannddlleess))  {{
                                                ///  LLoogg  ffeeaattuurreess  aatt  DDEEBBUUGG  lleevveell  ttoo  aavvooiidd  lloogg  ssppaamm                                                iiff  ffeeaattuurreess..rrssii..iiss__ssoommee(())  ||||  ffeeaattuurreess..mmaaccdd..iiss__ssoommee(())  {{
                                                        ttrraacciinngg::::ddeebbuugg!!((                                                                aasssseett  ==  ??ttiicckk..aasssseett,,                                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                                rrssii  ==  ??ffeeaattuurreess..rrssii,,                                                                mmaaccdd  ==  ??ffeeaattuurreess..mmaaccdd,,                                                                mmoommeennttuumm  ==  ??ffeeaattuurreess..mmoommeennttuumm,,                                                                ttrreenndd  ==  ??ffeeaattuurreess..ttrreenndd__ssttrreennggtthh,,                                                                ""üüììää  FFeeaattuurreess  ccoommppuutteedd""                                                        ));;
                                                        ///  SSeenndd  FFeeaattuurreess  ddiirreeccttllyy  ttoo  ssttrraatteeggyy                                                        iiff  lleett  EErrrr((ee))  ==  ffeeaattuurree__ttxx..sseenndd((ffeeaattuurreess))..aawwaaiitt  {{
                                                                ttrraacciinngg::::eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  sseenndd  ffeeaattuurreess""));;
                                                        }}
                                                }}
                                        }}
                                }}
                        }}
                }}
        }}
));;
        ///  SSttrraatteeggyy  eennggiinnee  ttaasskk  --  pprroocceesssseess  ffeeaattuurreess  aanndd  ggeenneerraatteess  ssiiggnnaallss        lleett  ssttrraatteeggyy__iinnnneerr  ==  ssttrraatteeggyy..cclloonnee(());;
        lleett  ssttrraatteeggyy__ppeerrssiisstteennccee  ==  ccssvv__ppeerrssiisstteennccee..cclloonnee(());;
        lleett  ssttrraatteeggyy__cclliieenntt  ==  cclloobb__cclliieenntt..cclloonnee(());;
  ///  FFoorr  mmaarrkkeett  llooookkuupp        lleett  ssttrraatteeggyy__rriisskk  ==  rriisskk__mmaannaaggeerr..cclloonnee(());;
  ///  FFoorr  ppoossiittiioonn  ssiizziinngg        lleett  ssttrraatteeggyy__kkeellllyy__ccffgg  ==  ccoonnffiigg..kkeellllyy..cclloonnee(());;
        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        lleett  ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy  ==  ddaasshhbbooaarrdd__mmeemmoorryy..cclloonnee(());;
        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        lleett  ssttrraatteeggyy__ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr  ==  ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr..cclloonnee(());;
        ///  CCiirrccuuiitt  bbrreeaakkeerr::  ttrraacckk  llaasstt  ffeeaattuurree  ttiimmeessttaammpp  ppeerr  aasssseett        lleett  llaasstt__ffeeaattuurree__ttss::  ssttdd::::ssyynncc::::AArrcc<<ttookkiioo::::ssyynncc::::MMuutteexx<<ssttdd::::ccoolllleeccttiioonnss::::HHaasshhMMaapp<<AAsssseett,,  ii6644>>>>>>  ==                ssttdd::::ssyynncc::::AArrcc::::nneeww((ttookkiioo::::ssyynncc::::MMuutteexx::::nneeww((ssttdd::::ccoolllleeccttiioonnss::::HHaasshhMMaapp::::nneeww(())))));;
        lleett  llaasstt__ffeeaattuurree__ttss__cclloonnee  ==  llaasstt__ffeeaattuurree__ttss..cclloonnee(());;
        lleett  ssttrraatteeggyy__hhaannddllee  ==  ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                uussee  ccrraattee::::ppeerrssiisstteennccee::::SSiiggnnaallRReeccoorrdd;;
                uussee  ccrraattee::::ppoollyymmaarrkkeett::::{{
                        ccoommppuuttee__ffrraaccttiioonnaall__kkeellllyy,,  eessttiimmaattee__eexxppeecctteedd__vvaalluuee,,  ffeeee__rraattee__ffrroomm__pprriiccee,,                }}
;;
                wwhhiillee  lleett  SSoommee((ffeeaattuurreess))  ==  ffeeaattuurree__rrxx..rreeccvv(())..aawwaaiitt  {{
                        ///  ‚‚îîÄÄ‚‚îîÄÄ  CCIIRRCCUUIITT  BBRREEAAKKEERR::  CChheecckk  ffoorr  ssttaallee  pprriiccee  ddaattaa  ‚‚îîÄÄ‚‚îîÄÄ                        lleett  nnooww  ==  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp__mmiilllliiss(());;
                        {{
                                lleett  mmuutt  ttss__mmaapp  ==  llaasstt__ffeeaattuurree__ttss__cclloonnee..lloocckk(())..aawwaaiitt;;
                                lleett  llaasstt__ttss  ==  ttss__mmaapp..ggeett((&&ffeeaattuurreess..aasssseett))..ccooppiieedd(())..uunnwwrraapp__oorr((00));;
                                ttss__mmaapp..iinnsseerrtt((ffeeaattuurreess..aasssseett,,  nnooww));;
                                ///  IIff  llaasstt  ffeeaattuurree  wwaass  mmoorree  tthhaann  6600  sseeccoonnddss  aaggoo,,  wwee  hhaadd  aa  ggaapp                                iiff  llaasstt__ttss  >>  00  &&&&  nnooww  --  llaasstt__ttss  >>  6600__000000  {{
                                        ttrraacciinngg::::wwaarrnn!!((                                                aasssseett  ==  ??ffeeaattuurreess..aasssseett,,                                                ggaapp__mmss  ==  nnooww  --  llaasstt__ttss,,                                                ""‚‚öö††ÔÔ∏∏èè  PPrriiccee  ddaattaa  ggaapp  ddeetteecctteedd  --  sskkiippppiinngg  ssiiggnnaall  ggeenneerraattiioonn""                                        ));;
                                        ccoonnttiinnuuee;;
                                }}
                        }}
                        ///  PPrroocceessss  ffeeaattuurreess  aanndd  ppootteennttiiaallllyy  ggeenneerraattee  ssiiggnnaall  ((uussiinngg  gglloobbaall  ssttrraatteeggyy  ffoorr  ccaalliibbrraattiioonn))                        iiff  lleett  SSoommee((ssiiggnnaall))  ==  ssttrraatteeggyy__iinnnneerr..lloocckk(())..aawwaaiitt..pprroocceessss((&&ffeeaattuurreess))  {{
                                ttrraacciinngg::::iinnffoo!!((                                        aasssseett  ==  ??ssiiggnnaall..aasssseett,,                                        ddiirreeccttiioonn  ==  ??ssiiggnnaall..ddiirreeccttiioonn,,                                        ccoonnffiiddeennccee  ==  %%ssiiggnnaall..ccoonnffiiddeennccee,,                                        rreeaassoonnss  ==  ??ssiiggnnaall..rreeaassoonnss,,                                        ""üüééØØ  SSiiggnnaall  ggeenneerraatteedd!!""                                ));;
                                ///  SSaavvee  ssiiggnnaall  ttoo  CCSSVV                                lleett  rreeccoorrdd  ==  SSiiggnnaallRReeccoorrdd  {{
                                        ttiimmeessttaammpp::  ssiiggnnaall..ttss,,                                        mmaarrkkeett__iidd::  ffoorrmmaatt!!((""{{
::??}}
--{{
::??}}
"",,  ssiiggnnaall..aasssseett,,  ssiiggnnaall..ttiimmeeffrraammee)),,                                        ddiirreeccttiioonn::  ffoorrmmaatt!!((""{{
::??}}
"",,  ssiiggnnaall..ddiirreeccttiioonn)),,                                        ccoonnffiiddeennccee::  ssiiggnnaall..ccoonnffiiddeennccee,,                                        eennttrryy__pprriiccee::  00..00,,  ///  WWiillll  bbee  sseett  oonn  eexxeeccuuttiioonn                                        ffeeaattuurreess__hhaasshh::  ffoorrmmaatt!!((                                                ""rrssii::{{
::..22}}
__mmaaccdd::{{
::..22}}
"",,                                                ffeeaattuurreess..rrssii..uunnwwrraapp__oorr((00..00)),,                                                ffeeaattuurreess..mmaaccdd..uunnwwrraapp__oorr((00..00))                                        )),,                                }}
;;
                                iiff  lleett  EErrrr((ee))  ==  ssttrraatteeggyy__ppeerrssiisstteennccee..ssaavvee__ssiiggnnaall((rreeccoorrdd))..aawwaaiitt  {{
                                        ttrraacciinngg::::wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  ssaavvee  ssiiggnnaall  ttoo  CCSSVV""));;
                                }}
                                ///  CCoonnvveerrtt  FFeeaattuurreess  ttoo  FFeeaattuurreeSSeett  ffoorr  SSiiggnnaall                                lleett  rreeggiimmee__ii88  ==  mmaattcchh  ffeeaattuurreess..rreeggiimmee  {{
                                        MMaarrkkeettRReeggiimmee::::TTrreennddiinngg  ==>>  11,,                                        MMaarrkkeettRReeggiimmee::::RRaannggiinngg  ==>>  00,,                                        MMaarrkkeettRReeggiimmee::::VVoollaattiillee  ==>>  --11,,                                }}
;;
                                lleett  ffeeaattuurree__sseett  ==  FFeeaattuurreeSSeett  {{
                                        ttss::  ffeeaattuurreess..ttss,,                                        aasssseett::  ffeeaattuurreess..aasssseett,,                                        ttiimmeeffrraammee::  ffeeaattuurreess..ttiimmeeffrraammee,,                                        rrssii::  ffeeaattuurreess..rrssii..uunnwwrraapp__oorr((5500..00)),,                                        mmaaccdd__lliinnee::  ffeeaattuurreess..mmaaccdd..uunnwwrraapp__oorr((00..00)),,                                        mmaaccdd__ssiiggnnaall::  ffeeaattuurreess..mmaaccdd__ssiiggnnaall..uunnwwrraapp__oorr((00..00)),,                                        mmaaccdd__hhiisstt::  ffeeaattuurreess..mmaaccdd__hhiisstt..uunnwwrraapp__oorr((00..00)),,                                        vvwwaapp::  ffeeaattuurreess..vvwwaapp..uunnwwrraapp__oorr((00..00)),,                                        bbbb__uuppppeerr::  ffeeaattuurreess..bbbb__uuppppeerr..uunnwwrraapp__oorr((00..00)),,                                        bbbb__lloowweerr::  ffeeaattuurreess..bbbb__lloowweerr..uunnwwrraapp__oorr((00..00)),,                                        aattrr::  ffeeaattuurreess..aattrr..uunnwwrraapp__oorr((00..00)),,                                        mmoommeennttuumm::  ffeeaattuurreess..mmoommeennttuumm..uunnwwrraapp__oorr((00..00)),,                                        mmoommeennttuumm__aacccceell::  ffeeaattuurreess..vveelloocciittyy..uunnwwrraapp__oorr((00..00)),,                                        bbooookk__iimmbbaallaannccee::  00..00,,                                        sspprreeaadd__bbppss::  00..00,,                                        ttrraaddee__iinntteennssiittyy::  00..00,,                                        hhaa__cclloossee::  ffeeaattuurreess..hhaa__cclloossee..uunnwwrraapp__oorr((00..00)),,                                        hhaa__ttrreenndd::  ffeeaattuurreess                                                ..hhaa__ttrreenndd                                                ..mmaapp((||dd||  iiff  dd  ====  DDiirreeccttiioonn::::UUpp  {{
  11  }}
  eellssee  {{
  --11  }}
))                                                ..uunnwwrraapp__oorr((00))  aass  ii88,,                                        oorraaccllee__ccoonnffiiddeennccee::  11..00,,                                        aaddxx::  ffeeaattuurreess..aaddxx..uunnwwrraapp__oorr((00..00)),,                                        ssttoocchh__rrssii::  ffeeaattuurreess..ssttoocchh__rrssii..uunnwwrraapp__oorr((00..55)),,                                        oobbvv::  ffeeaattuurreess..oobbvv..uunnwwrraapp__oorr((00..00)),,                                        rreellaattiivvee__vvoolluummee::  ffeeaattuurreess..rreellaattiivvee__vvoolluummee..uunnwwrraapp__oorr((11..00)),,                                        rreeggiimmee::  rreeggiimmee__ii88,,                                }}
;;
                                ///  LLooookk  uupp  mmaarrkkeett  ffoorr  tthhiiss  aasssseett//ttiimmeeffrraammee  ttoo  ggeett  eexxppiirryy  aanndd  ttookkeenn  iinnffoo                                lleett  sseelleecctteedd__mmaarrkkeett  ==  mmaattcchh  ssttrraatteeggyy__cclliieenntt                                        ..ffiinndd__ttrraaddeeaabbllee__mmaarrkkeett__ffoorr__ssiiggnnaall((ssiiggnnaall..aasssseett,,  ssiiggnnaall..ttiimmeeffrraammee))                                        ..aawwaaiitt                                {{
                                        SSoommee((mmaarrkkeett))  ==>>  mmaarrkkeett,,                                        NNoonnee  ==>>  {{
                                                ttrraacciinngg::::wwaarrnn!!((                                                        aasssseett  ==  ??ssiiggnnaall..aasssseett,,                                                        ttiimmeeffrraammee  ==  ??ssiiggnnaall..ttiimmeeffrraammee,,                                                        ""SSkkiippppiinngg  ssiiggnnaall::  nnoo  ttrraaddeeaabbllee  mmaarrkkeett  ffoouunndd""                                                ));;
                                                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy                                                        ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""mmaarrkkeett__nnoott__ffoouunndd""))                                                        ..aawwaaiitt;;
                                                ccoonnttiinnuuee;;
                                        }}
                                }}
;;
                                lleett  mmaarrkkeett__sslluugg  ==  sseelleecctteedd__mmaarrkkeett                                        ..sslluugg                                        ..cclloonnee(())                                        ..uunnwwrraapp__oorr__eellssee((||||  sseelleecctteedd__mmaarrkkeett..qquueessttiioonn..cclloonnee(())));;
                                lleett  ccoonnddiittiioonn__iidd  ==  sseelleecctteedd__mmaarrkkeett..ccoonnddiittiioonn__iidd..cclloonnee(());;
                                lleett  eexxppiirreess__aatt  ==  sseelleecctteedd__mmaarrkkeett                                        ..eenndd__ddaattee                                        ..aass__rreeff(())                                        ..aanndd__tthheenn((||dd||  ccrraattee::::cclloobb::::CClloobbCClliieenntt::::ppaarrssee__eexxppiirryy__ttoo__ttiimmeessttaammpp((dd))))                                        ..oorr__eellssee((||||  {{
                                                sseelleecctteedd__mmaarrkkeett                                                        ..eenndd__ddaattee__iissoo                                                        ..aass__rreeff(())                                                        ..aanndd__tthheenn((||dd||  ccrraattee::::cclloobb::::CClloobbCClliieenntt::::ppaarrssee__eexxppiirryy__ttoo__ttiimmeessttaammpp((dd))))                                        }}
))                                        ..uunnwwrraapp__oorr((00));;
                                lleett  ttookkeenn__iidd  ==  ccrraattee::::cclloobb::::CClloobbCClliieenntt::::rreessoollvvee__ttookkeenn__iidd__ffoorr__ddiirreeccttiioonn((                                        &&sseelleecctteedd__mmaarrkkeett,,                                        ssiiggnnaall..ddiirreeccttiioonn,,                                ))                                ..uunnwwrraapp__oorr__ddeeffaauulltt(());;
                                iiff  ttookkeenn__iidd..iiss__eemmppttyy(())  {{
                                        ttrraacciinngg::::wwaarrnn!!((                                                mmaarrkkeett__sslluugg  ==  %%mmaarrkkeett__sslluugg,,                                                ddiirreeccttiioonn  ==  ??ssiiggnnaall..ddiirreeccttiioonn,,                                                ""SSkkiippppiinngg  ssiiggnnaall::  ttookkeenn__iidd  nnoott  ffoouunndd  ffoorr  mmaarrkkeett  ddiirreeccttiioonn""                                        ));;
                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                        ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy                                                ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""ttookkeenn__nnoott__ffoouunndd""))                                                ..aawwaaiitt;;
                                        ccoonnttiinnuuee;;
                                }}
                                ///  CCoonnvveerrtt  ttoo  SSiiggnnaall  ttyyppee  uussiinngg  PPoollyymmaarrkkeett--nnaattiivvee  EEVV  ++  KKeellllyy  ssiizziinngg..                                lleett  qquuoottee  ==  mmaattcchh  ssttrraatteeggyy__cclliieenntt..qquuoottee__ttookkeenn((&&ttookkeenn__iidd))..aawwaaiitt  {{
                                        OOkk((qq))  iiff  qq..bbiidd  >>  00..00  &&&&  qq..aasskk  >>  00..00  &&&&  qq..mmiidd  >>  00..00  ==>>  qq,,                                        OOkk((__))  ==>>  {{
                                                ttrraacciinngg::::wwaarrnn!!((                                                        mmaarrkkeett__sslluugg  ==  %%mmaarrkkeett__sslluugg,,                                                        ttookkeenn__iidd  ==  %%ttookkeenn__iidd,,                                                        ""SSkkiippppiinngg  ssiiggnnaall::  iinnvvaalliidd  qquuoottee  vvaalluueess""                                                ));;
                                                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy                                                        ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""qquuoottee__iinnvvaalliidd""))                                                        ..aawwaaiitt;;
                                                ccoonnttiinnuuee;;
                                        }}
                                        EErrrr((ee))  ==>>  {{
                                                ttrraacciinngg::::wwaarrnn!!((                                                        mmaarrkkeett__sslluugg  ==  %%mmaarrkkeett__sslluugg,,                                                        ttookkeenn__iidd  ==  %%ttookkeenn__iidd,,                                                        eerrrroorr  ==  %%ee,,                                                        ""SSkkiippppiinngg  ssiiggnnaall::  ffaaiilleedd  ttoo  ffeettcchh  ttookkeenn  qquuoottee""                                                ));;
                                                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy                                                        ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""qquuoottee__ffeettcchh__eerrrroorr""))                                                        ..aawwaaiitt;;
                                                ccoonnttiinnuuee;;
                                        }}
                                }}
;;
                                lleett  pp__mmaarrkkeett  ==  qquuoottee..mmiidd..ccllaammpp((00..0011,,  00..9999));;
                                lleett  sspprreeaadd  ==  qquuoottee..sspprreeaadd..mmaaxx((00..00));;
                                lleett  mmaaxx__sspprreeaadd  ==  mmaattcchh  ssiiggnnaall..ttiimmeeffrraammee  {{
                                        TTiimmeeffrraammee::::MMiinn1155  ==>>  00..0033,,                                        TTiimmeeffrraammee::::HHoouurr11  ==>>  00..0055,,                                }}
;;
                                iiff  sspprreeaadd  >>  mmaaxx__sspprreeaadd  {{
                                        ttrraacciinngg::::iinnffoo!!((                                                mmaarrkkeett__sslluugg  ==  %%mmaarrkkeett__sslluugg,,                                                ttookkeenn__iidd  ==  %%ttookkeenn__iidd,,                                                sspprreeaadd  ==  sspprreeaadd,,                                                mmaaxx__sspprreeaadd  ==  mmaaxx__sspprreeaadd,,                                                ""SSkkiippppiinngg  ssiiggnnaall::  sspprreeaadd  ttoooo  wwiiddee  ffoorr  ttiimmeeffrraammee  ppoolliiccyy""                                        ));;
                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                        ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy                                                ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""sspprreeaadd__ttoooo__wwiiddee""))                                                ..aawwaaiitt;;
                                        ccoonnttiinnuuee;;
                                }}
                                lleett  mmiinn__ddeepptthh__ttoopp55  ==  mmaattcchh  ssiiggnnaall..ttiimmeeffrraammee  {{
                                        TTiimmeeffrraammee::::MMiinn1155  ==>>  5500..00,,                                        TTiimmeeffrraammee::::HHoouurr11  ==>>  2255..00,,                                }}
;;
                                iiff  qquuoottee..ddeepptthh__ttoopp55  >>  00..00  &&&&  qquuoottee..ddeepptthh__ttoopp55  <<  mmiinn__ddeepptthh__ttoopp55  {{
                                        ttrraacciinngg::::iinnffoo!!((                                                mmaarrkkeett__sslluugg  ==  %%mmaarrkkeett__sslluugg,,                                                ttookkeenn__iidd  ==  %%ttookkeenn__iidd,,                                                ddeepptthh__ttoopp55  ==  qquuoottee..ddeepptthh__ttoopp55,,                                                mmiinn__ddeepptthh__ttoopp55  ==  mmiinn__ddeepptthh__ttoopp55,,                                                ""SSkkiippppiinngg  ssiiggnnaall::  ddeepptthh  bbeellooww  lliiqquuiiddiittyy  ppoolliiccyy""                                        ));;
                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                        ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy                                                ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""ddeepptthh__ttoooo__llooww""))                                                ..aawwaaiitt;;
                                        ccoonnttiinnuuee;;
                                }}
                                lleett  pp__mmooddeell  ==  ssiiggnnaall..ccoonnffiiddeennccee..ccllaammpp((00..0011,,  00..9999));;
                                lleett  ffeeee__rraattee  ==  ffeeee__rraattee__ffrroomm__pprriiccee((pp__mmaarrkkeett));;
                                lleett  eevv  ==                                        eessttiimmaattee__eexxppeecctteedd__vvaalluuee((pp__mmaarrkkeett,,  pp__mmooddeell,,  pp__mmaarrkkeett,,  ffeeee__rraattee,,  sspprreeaadd,,  00..000055));;
                                iiff  eevv..eeddggee__nneett  <<==  00..00  {{
                                        ttrraacciinngg::::iinnffoo!!((                                                mmaarrkkeett__sslluugg  ==  %%mmaarrkkeett__sslluugg,,                                                pp__mmaarrkkeett  ==  pp__mmaarrkkeett,,                                                pp__mmooddeell  ==  pp__mmooddeell,,                                                eeddggee__nneett  ==  eevv..eeddggee__nneett,,                                                ""SSkkiippppiinngg  ssiiggnnaall  dduuee  ttoo  nnoonn--ppoossiittiivvee  PPoollyymmaarrkkeett  eeddggee""                                        ));;
                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                        ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy                                                ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""eeddggee__nnoonn__ppoossiittiivvee""))                                                ..aawwaaiitt;;
                                        ccoonnttiinnuuee;;
                                }}
                                lleett  ((kkeellllyy__ffrraaccttiioonn,,  ccaapp))  ==  mmaattcchh  ssiiggnnaall..ttiimmeeffrraammee  {{
                                        TTiimmeeffrraammee::::MMiinn1155  ==>>  ((                                                ssttrraatteeggyy__kkeellllyy__ccffgg..ffrraaccttiioonn__1155mm,,                                                ssttrraatteeggyy__kkeellllyy__ccffgg..mmaaxx__bbaannkkrroollll__ffrraaccttiioonn__1155mm,,                                        )),,                                        TTiimmeeffrraammee::::HHoouurr11  ==>>  ((                                                ssttrraatteeggyy__kkeellllyy__ccffgg..ffrraaccttiioonn__11hh,,                                                ssttrraatteeggyy__kkeellllyy__ccffgg..mmaaxx__bbaannkkrroollll__ffrraaccttiioonn__11hh,,                                        )),,                                }}
;;
                                lleett  kkeellllyy  ==  ccoommppuuttee__ffrraaccttiioonnaall__kkeellllyy((pp__mmooddeell,,  00..0055,,  pp__mmaarrkkeett,,  kkeellllyy__ffrraaccttiioonn,,  ccaapp));;
                                lleett  ffaallllbbaacckk__ssiizzee  ==  ssttrraatteeggyy__rriisskk..ccaallccuullaattee__ssiizzee__ffrroomm__ccoonnffiiddeennccee((ssiiggnnaall..ccoonnffiiddeennccee));;
                                lleett  bbaallaannccee  ==  ssttrraatteeggyy__rriisskk..ggeett__bbaallaannccee(());;
                                lleett  bbaannkkrroollll  ==  iiff  bbaallaannccee  >>  00..00  {{
  bbaallaannccee  }}
  eellssee  {{
  11000000..00  }}
;;
                                lleett  kkeellllyy__ssiizzee  ==  bbaannkkrroollll  **  kkeellllyy..ff__ffrraaccttiioonnaall;;
                                lleett  ccaallccuullaatteedd__ssiizzee  ==  iiff  ssttrraatteeggyy__kkeellllyy__ccffgg..eennaabblleedd  &&&&  kkeellllyy__ssiizzee  >>==  11..00  {{
                                        kkeellllyy__ssiizzee                                }}
  eellssee  {{
                                        ffaallllbbaacckk__ssiizzee                                }}
;;
                                lleett  ssiigg  ==  SSiiggnnaall  {{
                                        iidd::  uuuuiidd::::UUuuiidd::::nneeww__vv44(())..ttoo__ssttrriinngg(()),,                                        ttss::  ssiiggnnaall..ttss,,                                        aasssseett::  ssiiggnnaall..aasssseett,,                                        ttiimmeeffrraammee::  ssiiggnnaall..ttiimmeeffrraammee,,                                        ddiirreeccttiioonn::  ssiiggnnaall..ddiirreeccttiioonn,,                                        ccoonnffiiddeennccee::  ssiiggnnaall..ccoonnffiiddeennccee,,                                        ffeeaattuurreess::  ffeeaattuurree__sseett,,                                        ssttrraatteeggyy__iidd::  ""rruulleess__vv11""..ttoo__ssttrriinngg(()),,                                        mmaarrkkeett__sslluugg,,                                        ccoonnddiittiioonn__iidd,,                                        ttookkeenn__iidd,,                                        eexxppiirreess__aatt,,                                        ssuuggggeesstteedd__ssiizzee__uussddcc::  ccaallccuullaatteedd__ssiizzee,,                                        iinnddiiccaattoorrss__uusseedd::  ssiiggnnaall..iinnddiiccaattoorrss__uusseedd,,                                }}
;;
                                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                lleett  ddaasshhbbooaarrdd__ssiiggnnaall  ==  ccrraattee::::ddaasshhbbooaarrdd::::SSiiggnnaallRReessppoonnssee  {{
                                        ttiimmeessttaammpp::  ssiigg..ttss,,                                        ssiiggnnaall__iidd::  ssiigg..iidd..cclloonnee(()),,                                        aasssseett::  ffoorrmmaatt!!((""{{
::??}}
"",,  ssiigg..aasssseett)),,                                        ttiimmeeffrraammee::  ffoorrmmaatt!!((""{{
}}
"",,  ssiigg..ttiimmeeffrraammee)),,                                        ddiirreeccttiioonn::  ffoorrmmaatt!!((""{{
::??}}
"",,  ssiigg..ddiirreeccttiioonn)),,                                        ccoonnffiiddeennccee::  ssiigg..ccoonnffiiddeennccee,,                                        eennttrryy__pprriiccee::  00..00,,                                        mmaarrkkeett__sslluugg::  ssiigg..mmaarrkkeett__sslluugg..cclloonnee(()),,                                        eexxppiirreess__aatt::  ssiigg..eexxppiirreess__aatt,,                                }}
;;
                                iiff  lleett  EErrrr((ee))  ==  ssiiggnnaall__ttxx..sseenndd((ssiigg))..aawwaaiitt  {{
                                        ttrraacciinngg::::eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  sseenndd  ssiiggnnaall""));;
                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                        ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy                                                ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""ssiiggnnaall__cchhaannnneell__sseenndd__eerrrroorr""))                                                ..aawwaaiitt;;
                                }}
  eellssee  {{
                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                        {{
                                                ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy                                                        ..rreeccoorrdd__eexxeeccuuttiioonn__aacccceepptt(())                                                        ..aawwaaiitt;;
                                                ssttrraatteeggyy__ddaasshhbbooaarrdd__mmeemmoorryy..aadddd__ssiiggnnaall((ddaasshhbbooaarrdd__ssiiggnnaall..cclloonnee(())))..aawwaaiitt;;
                                                ssttrraatteeggyy__ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr..bbrrooaaddccaasstt__ssiiggnnaall((ddaasshhbbooaarrdd__ssiiggnnaall));;
                                        }}
                                }}
                        }}
                }}
        }}
));;
        ///  OOrrddeerr  eexxeeccuuttiioonn  ttaasskk  --  ccoonnssuummeess  oorrddeerr__rrxx  aanndd  ssuubbmmiittss  ttoo  PPoollyymmaarrkkeett        lleett  eexxeeccuuttiioonn__cclliieenntt  ==  cclloobb__cclliieenntt..cclloonnee(());;
        lleett  eexxeeccuuttiioonn__hhaannddllee  ==  ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                iiff  lleett  EErrrr((ee))  ==  eexxeeccuuttiioonn__cclliieenntt..rruunn((oorrddeerr__rrxx))..aawwaaiitt  {{
                        eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""EExxeeccuuttiioonn  cclliieenntt  ttaasskk  ffaaiilleedd""));;
                }}
        }}
));;
        ///  ‚‚îîÄÄ‚‚îîÄÄ  LLiivvee  ppoossiittiioonn  ‚‚ÜÜíí  iinnddiiccaattoorrss  ttrraacckkiinngg  ((ffoorr  ccaalliibbrraattiioonn  iinn  lliivvee  mmooddee))  ‚‚îîÄÄ‚‚îîÄÄ        ///  WWhheenn  aa  lliivvee  ssiiggnnaall  iiss  eexxeeccuutteedd,,  wwee  ssttoorree  wwhhiicchh  iinnddiiccaattoorrss  ggeenneerraatteedd  iitt..        ///  WWhheenn  tthhee  ppoossiittiioonn  cclloosseess  ((ddeetteecctteedd  bbyy  ppoossiittiioonn  mmoonniittoorr)),,  wwee  uussee  tthhiiss  ttoo        ///  ffeeeedd  bbaacckk  iinnttoo  tthhee  ccaalliibbrraattoorr  ‚‚ÄÄîî  ssoo  tthhee  bbrraaiinn  lleeaarrnnss  ffrroomm  lliivvee  ttrraaddeess  ttoooo..        lleett  lliivvee__ppoossiittiioonn__iinnddiiccaattoorrss::  AArrcc<<                MMuutteexx<<ssttdd::::ccoolllleeccttiioonnss::::HHaasshhMMaapp<<((AAsssseett,,  TTiimmeeffrraammee)),,  ((VVeecc<<SSttrriinngg>>,,  ff6644))>>>>,,        >>  ==  AArrcc::::nneeww((MMuutteexx::::nneeww((ssttdd::::ccoolllleeccttiioonnss::::HHaasshhMMaapp::::nneeww(())))));;
        lleett  lliivvee__iinnddiiccaattoorrss__ffoorr__mmoonniittoorr  ==  lliivvee__ppoossiittiioonn__iinnddiiccaattoorrss..cclloonnee(());;
        lleett  lliivvee__iinnddiiccaattoorrss__ffoorr__mmaaiinn  ==  lliivvee__ppoossiittiioonn__iinnddiiccaattoorrss..cclloonnee(());;
        lleett  lliivvee__wwiinnddooww__bbiiaass::  AArrcc<<                MMuutteexx<<ssttdd::::ccoolllleeccttiioonnss::::HHaasshhMMaapp<<((AAsssseett,,  TTiimmeeffrraammee,,  ii6644)),,  DDiirreeccttiioonn>>>>,,        >>  ==  AArrcc::::nneeww((MMuutteexx::::nneeww((ssttdd::::ccoolllleeccttiioonnss::::HHaasshhMMaapp::::nneeww(())))));;
        ///  SSttrraatteeggyy  ++  ccaalliibbrraattoorr  ssttaattee  ppaatthh  ffoorr  lliivvee  ccaalliibbrraattiioonn  ssaavviinngg        lleett  ssttrraatteeggyy__ffoorr__lliivvee__ccaalliibbrraattiioonn  ==  ssttrraatteeggyy..cclloonnee(());;
        lleett  ccaalliibbrraattoorr__ssaavvee__ppaatthh__lliivvee  ==  ccaalliibbrraattoorr__ssttaattee__ffiillee__vv22..cclloonnee(());;
        ///  PPoossiittiioonn  mmoonniittoorriinngg  ttaasskk  --  ffeettcchheess  wwaalllleett  ppoossiittiioonnss  ffoorr  TTPP//SSLL        lleett  ppoossiittiioonn__cclliieenntt  ==  cclloobb__cclliieenntt..cclloonnee(());;
        lleett  ppoossiittiioonn__rriisskk  ==  rriisskk__mmaannaaggeerr..cclloonnee(());;
        lleett  ppoossiittiioonn__ttrraacckkeerr  ==  bbaallaannccee__ttrraacckkeerr..cclloonnee(());;
        lleett  rreeddeeeemmeedd__ccllaaiimmss  ==  AArrcc::::nneeww((ttookkiioo::::ssyynncc::::MMuutteexx::::nneeww((                ssttdd::::ccoolllleeccttiioonnss::::HHaasshhSSeett::::<<SSttrriinngg>>::::nneeww(()),,        ))));;
        lleett  rreeddeeeemmeedd__ccllaaiimmss__ffoorr__mmoonniittoorr  ==  rreeddeeeemmeedd__ccllaaiimmss..cclloonnee(());;
        lleett  wwaalllleett__aaddddrreessss  ==  ssttdd::::eennvv::::vvaarr((""PPOOLLYYMMAARRKKEETT__WWAALLLLEETT""))                ..ookk(())                ..oorr__eellssee((||||  ssttdd::::eennvv::::vvaarr((""PPOOLLYYMMAARRKKEETT__AADDDDRREESSSS""))..ookk(())))                ..uunnwwrraapp__oorr__ddeeffaauulltt(());;
        lleett  ppoossiittiioonn__hhaannddllee  ==  ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                lleett  mmuutt  iinntteerrvvaall  ==  ttookkiioo::::ttiimmee::::iinntteerrvvaall((ttookkiioo::::ttiimmee::::DDuurraattiioonn::::ffrroomm__sseeccss((3300))));;
                lloooopp  {{
                        iinntteerrvvaall..ttiicckk(())..aawwaaiitt;;
                        iiff  wwaalllleett__aaddddrreessss..iiss__eemmppttyy(())  {{
                                ccoonnttiinnuuee;;
                        }}
                        mmaattcchh  ppoossiittiioonn__cclliieenntt                                ..ffeettcchh__wwaalllleett__ppoossiittiioonnss((&&wwaalllleett__aaddddrreessss))                                ..aawwaaiitt                        {{
                                OOkk((ppoossiittiioonnss))  ==>>  {{
                                        ffoorr  ppooss  iinn  ppoossiittiioonnss  {{
                                                ///  PPaarrssee  ppoossiittiioonn  ssiizzee                                                lleett  ssiizzee::  ff6644  ==  ppooss..ssiizzee..ppaarrssee(())..uunnwwrraapp__oorr((00..00));;
                                                iiff  ssiizzee  ====  00..00  {{
                                                        ccoonnttiinnuuee;;
                                                }}
                                                lleett  aavvgg__pprriiccee::  ff6644  ==  ppooss..aavvgg__pprriiccee..ppaarrssee(())..uunnwwrraapp__oorr((00..00));;
                                                lleett  ccuurrrreenntt__pprriiccee::  ff6644  ==  ppooss                                                        ..ccuurrrreenntt__pprriiccee                                                        ..aass__rreeff(())                                                        ..aanndd__tthheenn((||pp||  pp..ppaarrssee(())..ookk(())))                                                        ..uunnwwrraapp__oorr((aavvgg__pprriiccee));;
                                                ///  TTrryy  ttoo  ppaarrssee  aasssseett  ffrroomm  ppoossiittiioonn                                                lleett  aasssseett__ssttrr  ==  ppooss..aasssseett..ttoo__uuppppeerrccaassee(());;
                                                lleett  aasssseett  ==  iiff  aasssseett__ssttrr..ccoonnttaaiinnss((""BBTTCC""))  {{
                                                        SSoommee((AAsssseett::::BBTTCC))                                                }}
  eellssee  iiff  aasssseett__ssttrr..ccoonnttaaiinnss((""EETTHH""))  {{
                                                        SSoommee((AAsssseett::::EETTHH))                                                }}
  eellssee  iiff  aasssseett__ssttrr..ccoonnttaaiinnss((""SSOOLL""))  {{
                                                        SSoommee((AAsssseett::::SSOOLL))                                                }}
  eellssee  iiff  aasssseett__ssttrr..ccoonnttaaiinnss((""XXRRPP""))  {{
                                                        SSoommee((AAsssseett::::XXRRPP))                                                }}
  eellssee  {{
                                                        NNoonnee                                                }}
;;
                                                ///  UUppddaattee  rriisskk  mmaannaaggeerr  wwiitthh  ppoossiittiioonn  ((oonnllyy  iiff  wwee  ccaann  iiddeennttiiffyy  tthhee  aasssseett))                                                iiff  lleett  SSoommee((aasssseett))  ==  aasssseett  {{
                                                        iiff  lleett  SSoommee((eexxiitt__rreeaassoonn))  ==                                                                ppoossiittiioonn__rriisskk..uuppddaattee__ppoossiittiioonn((aasssseett,,  ccuurrrreenntt__pprriiccee))                                                        {{
                                                                ttrraacciinngg::::wwaarrnn!!((                                                                        aasssseett  ==  ??aasssseett,,                                                                        rreeaassoonn  ==  ??eexxiitt__rreeaassoonn,,                                                                        ccuurrrreenntt__pprriiccee  ==  ccuurrrreenntt__pprriiccee,,                                                                        ""üüöö®®  PPoossiittiioonn  sshhoouulldd  bbee  cclloosseedd!!""                                                                ));;
                                                                ///  CCaallccuullaattee  PPnnLL  ffoorr  tthhiiss  ppoossiittiioonn                                                                ///  FFoorr  aa  LLOONNGG::  ppnnll  ==  ((ccuurrrreenntt__pprriiccee  --  aavvgg__pprriiccee))  **  ssiizzee                                                                ///  FFoorr  aa  SSHHOORRTT::  ppnnll  ==  ((aavvgg__pprriiccee  --  ccuurrrreenntt__pprriiccee))  **  ssiizzee                                                                ///  AAssssuummiinngg  wwee''rree  aallwwaayyss  LLOONNGG  ffoorr  pprreeddiiccttiioonn  mmaarrkkeettss                                                                lleett  ppnnll  ==  ((ccuurrrreenntt__pprriiccee  --  aavvgg__pprriiccee))  **  ssiizzee;;
                                                                lleett  iinntteerrnnaall__rreessuulltt  ==  iiff  ppnnll  >>==  00..00  {{
  ""WWIINN""  }}
  eellssee  {{
  ""LLOOSSSS""  }}
;;
                                                                ///  CCrreeaattee  wwiinn//lloossss  rreeccoorrdd                                                                uussee  ccrraattee::::ppeerrssiisstteennccee::::WWiinnLLoossssRReeccoorrdd;;
                                                                lleett  ttookkeenn__iidd  ==                                                                        ppooss..ttookkeenn__iidd..cclloonnee(())..uunnwwrraapp__oorr__eellssee((||||  ppooss..aasssseett..cclloonnee(())));;
                                                                lleett  ooffffiicciiaall__rreessuulltt  ==                                                                        iiff  lleett  SSoommee((ccoonnddiittiioonn__iidd))  ==  ppooss..ccoonnddiittiioonn__iidd..aass__ddeerreeff(())  {{
                                                                                ppoossiittiioonn__cclliieenntt                                                                                        ..ooffffiicciiaall__rreessuulltt__ffoorr__ttookkeenn((ccoonnddiittiioonn__iidd,,  &&ttookkeenn__iidd))                                                                                        ..aawwaaiitt                                                                        }}
  eellssee  {{
                                                                                NNoonnee                                                                        }}
;;
                                                                iiff  ooffffiicciiaall__rreessuulltt..aass__ddeerreeff(())  ====  SSoommee((""WWIINN""))  {{
                                                                        iiff  lleett  SSoommee((ccoonnddiittiioonn__iidd))  ==  ppooss..ccoonnddiittiioonn__iidd..aass__ddeerreeff(())  {{
                                                                                lleett  rreeddeeeemm__kkeeyy  ==  ffoorrmmaatt!!((""{{
}}
::{{
}}
"",,  ccoonnddiittiioonn__iidd,,  ttookkeenn__iidd));;
                                                                                lleett  sshhoouulldd__aatttteemmpptt  ==  {{
                                                                                        lleett  mmuutt  rreeddeeeemmeedd  ==                                                                                                rreeddeeeemmeedd__ccllaaiimmss__ffoorr__mmoonniittoorr..lloocckk(())..aawwaaiitt;;
                                                                                        rreeddeeeemmeedd..iinnsseerrtt((rreeddeeeemm__kkeeyy..cclloonnee(())))                                                                                }}
;;
                                                                                iiff  sshhoouulldd__aatttteemmpptt  {{
                                                                                        iiff  lleett  EErrrr((ee))  ==  ppoossiittiioonn__cclliieenntt                                                                                                ..rreeddeeeemm__wwiinnnniinngg__ttookkeennss((                                                                                                        ccoonnddiittiioonn__iidd,,                                                                                                        &&ttookkeenn__iidd,,                                                                                                        ssiizzee,,                                                                                                ))                                                                                                ..aawwaaiitt                                                                                        {{
                                                                                                ttrraacciinngg::::wwaarrnn!!((                                                                                                        eerrrroorr  ==  %%ee,,                                                                                                        ccoonnddiittiioonn__iidd  ==  %%ccoonnddiittiioonn__iidd,,                                                                                                        ttookkeenn__iidd  ==  %%ttookkeenn__iidd,,                                                                                                        ""RReeddeemmppttiioonn  aatttteemmpptt  ffaaiilleedd""                                                                                                ));;
                                                                                                rreeddeeeemmeedd__ccllaaiimmss__ffoorr__mmoonniittoorr                                                                                                        ..lloocckk(())                                                                                                        ..aawwaaiitt                                                                                                        ..rreemmoovvee((&&rreeddeeeemm__kkeeyy));;
                                                                                        }}
                                                                                }}
                                                                        }}
                                                                }}
                                                                lleett  rreeccoorrdd  ==  WWiinnLLoossssRReeccoorrdd  {{
                                                                        ttiimmeessttaammpp::  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp(()),,                                                                        mmaarrkkeett__sslluugg::  ffoorrmmaatt!!((""{{
::??}}
"",,  aasssseett)),,                                                                        ttookkeenn__iidd,,                                                                        eennttrryy__pprriiccee::  aavvgg__pprriiccee,,                                                                        eexxiitt__pprriiccee::  ccuurrrreenntt__pprriiccee,,                                                                        ssiizzee,,                                                                        ppnnll,,                                                                        iinntteerrnnaall__rreessuulltt::  iinntteerrnnaall__rreessuulltt..ttoo__ssttrriinngg(()),,                                                                        eexxiitt__rreeaassoonn::  eexxiitt__rreeaassoonn..ttoo__ssttrriinngg(()),,                                                                        ooffffiicciiaall__rreessuulltt,,                                                                }}
;;
                                                                ///  RReeccoorrdd  wwiinn//lloossss  iinn  bbaallaannccee  ttrraacckkeerr                                                                ppoossiittiioonn__ttrraacckkeerr..rreeccoorrdd__wwiinnlloossss((rreeccoorrdd));;
                                                                ///  ‚‚îîÄÄ‚‚îîÄÄ  LLIIVVEE  CCAALLIIBBRRAATTIIOONN::  TTrraaiinn  tthhee  bbrraaiinn  ffrroomm  lliivvee  ttrraaddee  rreessuullttss  ‚‚îîÄÄ‚‚îîÄÄ                                                                ///  LLooookk  uupp  wwhhiicchh  iinnddiiccaattoorrss  ggeenneerraatteedd  tthhiiss  ppoossiittiioonn''ss  ssiiggnnaall                                                                lleett  ppaarrsseedd__ttiimmeeffrraammee  ==  ppaarrssee__ttiimmeeffrraammee__ffrroomm__mmaarrkkeett__tteexxtt((&&ppooss..aasssseett));;
                                                                lleett  ((ttiimmeeffrraammee,,  iinnddiiccaattoorrss,,  pp__mmooddeell))  ==  {{
                                                                        lleett  mmuutt  ppeennddiinngg  ==  lliivvee__iinnddiiccaattoorrss__ffoorr__mmoonniittoorr..lloocckk(())..aawwaaiitt;;
                                                                        iiff  lleett  SSoommee((ttff))  ==  ppaarrsseedd__ttiimmeeffrraammee  {{
                                                                                ppeennddiinngg                                                                                        ..rreemmoovvee((&&((aasssseett,,  ttff))))                                                                                        ..mmaapp((||ccttxx||  ((ttff,,  ccttxx..00,,  ccttxx..11))))                                                                                        ..uunnwwrraapp__oorr((((ttff,,  VVeecc::::nneeww(()),,  00..55))))                                                                        }}
  eellssee  {{
                                                                                lleett  kkeeyyss::  VVeecc<<((AAsssseett,,  TTiimmeeffrraammee))>>  ==  ppeennddiinngg                                                                                        ..kkeeyyss(())                                                                                        ..ccooppiieedd(())                                                                                        ..ffiilltteerr((||((aa,,  __))||  **aa  ====  aasssseett))                                                                                        ..ccoolllleecctt(());;
                                                                                iiff  kkeeyyss..lleenn(())  ====  11  {{
                                                                                        lleett  kkeeyy  ==  kkeeyyss[[00]];;
                                                                                        lleett  ((iinnddss,,  pp__mmooddeell))  ==                                                                                                ppeennddiinngg..rreemmoovvee((&&kkeeyy))..uunnwwrraapp__oorr((((VVeecc::::nneeww(()),,  00..55))));;
                                                                                        ((kkeeyy..11,,  iinnddss,,  pp__mmooddeell))                                                                                }}
  eellssee  {{
                                                                                        iiff  kkeeyyss..lleenn(())  >>  11  {{
                                                                                                ttrraacciinngg::::wwaarrnn!!((                                                                                                        aasssseett  ==  ??aasssseett,,                                                                                                        ccaannddiiddaatteess  ==  kkeeyyss..lleenn(()),,                                                                                                        ""AAmmbbiigguuoouuss  lliivvee  ccaalliibbrraattiioonn  ccoonntteexxtt;;
  sskkiippppiinngg  ffeeeeddbbaacckk""                                                                                                ));;
                                                                                        }}
                                                                                        ((TTiimmeeffrraammee::::MMiinn1155,,  VVeecc::::nneeww(()),,  00..55))                                                                                }}
                                                                        }}
                                                                }}
;;
                                                                iiff  !!iinnddiiccaattoorrss..iiss__eemmppttyy(())  {{
                                                                        lleett  iiss__wwiinn  ==  ppnnll  >>==  00..00;;
                                                                        lleett  rreessuulltt  ==  iiff  iiss__wwiinn  {{
                                                                                TTrraaddeeRReessuulltt::::WWiinn                                                                        }}
  eellssee  {{
                                                                                TTrraaddeeRReessuulltt::::LLoossss                                                                        }}
;;
                                                                        lleett  mmuutt  ssttrraatt  ==  ssttrraatteeggyy__ffoorr__lliivvee__ccaalliibbrraattiioonn..lloocckk(())..aawwaaiitt;;
                                                                        ssttrraatt..rreeccoorrdd__ttrraaddee__wwiitthh__iinnddiiccaattoorrss__ffoorr__mmaarrkkeett((                                                                                aasssseett,,                                                                                ttiimmeeffrraammee,,                                                                                &&iinnddiiccaattoorrss,,                                                                                rreessuulltt,,                                                                        ));;
                                                                        ssttrraatt..rreeccoorrdd__pprreeddiiccttiioonn__oouuttccoommee__ffoorr__mmaarrkkeett((                                                                                aasssseett,,  ttiimmeeffrraammee,,  pp__mmooddeell,,  iiss__wwiinn,,                                                                        ));;
                                                                        ///  SSaavvee  ccaalliibbrraattoorr  ssttaattee  ttoo  ddiisskk                                                                        lleett  ssttaattss  ==  ssttrraatt..eexxppoorrtt__ccaalliibbrraattoorr__ssttaattee__vv22(());;
                                                                        lleett  ttoottaall  ==  ssttrraatt..ccaalliibbrraattoorr__ttoottaall__ttrraaddeess(());;
                                                                        lleett  ccaalliibbrraatteedd  ==  ssttrraatt..iiss__ccaalliibbrraatteedd(());;
                                                                        ddrroopp((ssttrraatt));;
                                                                        iiff  lleett  OOkk((jjssoonn))  ==  sseerrddee__jjssoonn::::ttoo__ssttrriinngg__pprreettttyy((&&ssttaattss))  {{
                                                                                iiff  lleett  EErrrr((ee))  ==                                                                                        ssttdd::::ffss::::wwrriittee((&&ccaalliibbrraattoorr__ssaavvee__ppaatthh__lliivvee,,  &&jjssoonn))                                                                                {{
                                                                                        ttrraacciinngg::::wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  ssaavvee  ccaalliibbrraattoorr  ssttaattee  ((lliivvee))""));;
                                                                                }}
                                                                        }}
                                                                        ttrraacciinngg::::iinnffoo!!((                                                                                aasssseett  ==  ??aasssseett,,                                                                                ttiimmeeffrraammee  ==  ??ttiimmeeffrraammee,,                                                                                iiss__wwiinn  ==  iiss__wwiinn,,                                                                                pp__mmooddeell  ==  pp__mmooddeell,,                                                                                iinnddiiccaattoorrss__ccoouunntt  ==  iinnddiiccaattoorrss..lleenn(()),,                                                                                ttoottaall__ttrraaddeess  ==  ttoottaall,,                                                                                ccaalliibbrraatteedd  ==  ccaalliibbrraatteedd,,                                                                                ""üüßß††  [[LLIIVVEE  CCAALLIIBBRRAATTIIOONN]]  TTrraaddee  rreessuulltt  rreeccoorrddeedd  &&  ssaavveedd""                                                                        ));;
                                                                }}
                                                                ttrraacciinngg::::iinnffoo!!((                                                                        aasssseett  ==  ??aasssseett,,                                                                        ppnnll  ==  ppnnll,,                                                                        rreessuulltt  ==  iinntteerrnnaall__rreessuulltt,,                                                                        ""üüììää  WWiinn//LLoossss  rreeccoorrddeedd""                                                                ));;
                                                                ///  IInn  aa  rreeaall  iimmpplleemmeennttaattiioonn,,  tthhiiss  wwoouulldd  ttrriiggggeerr  aa  cclloossee  oorrddeerr                                                        }}
                                                }}
                                                ttrraacciinngg::::iinnffoo!!((                                                        aasssseett  ==  %%ppooss..aasssseett,,                                                        ssiizzee  ==  ssiizzee,,                                                        aavvgg__pprriiccee  ==  aavvgg__pprriiccee,,                                                        ccuurrrreenntt  ==  ccuurrrreenntt__pprriiccee,,                                                        ""üüììää  PPoossiittiioonn  ttrraacckkeedd""                                                ));;
                                        }}
                                }}
                                EErrrr((ee))  ==>>  {{
                                        ttrraacciinngg::::wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  ffeettcchh  wwaalllleett  ppoossiittiioonnss""));;
                                }}
                        }}
                }}
        }}
));;
        ///  BBaallaannccee  ttrraacckkiinngg  ttaasskk  --  ffeettcchheess  bbaallaannccee  ppeerriiooddiiccaallllyy  aanndd  uuppddaatteess  ttrraacckkeerrss        lleett  bbaallaannccee__cclliieenntt  ==  cclloobb__cclliieenntt..cclloonnee(());;
        lleett  bbaallaannccee__ttrraacckkeerr__cclloonnee  ==  bbaallaannccee__ttrraacckkeerr..cclloonnee(());;
        lleett  bbaallaannccee__rriisskk  ==  rriisskk__mmaannaaggeerr..cclloonnee(());;
        lleett  bbaallaannccee__wwaalllleett__aaddddrreessss  ==  ssttdd::::eennvv::::vvaarr((""PPOOLLYYMMAARRKKEETT__WWAALLLLEETT""))                ..ookk(())                ..oorr__eellssee((||||  ssttdd::::eennvv::::vvaarr((""PPOOLLYYMMAARRKKEETT__AADDDDRREESSSS""))..ookk(())))                ..uunnwwrraapp__oorr__ddeeffaauulltt(());;
        lleett  bbaallaannccee__hhaannddllee  ==  ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                lleett  mmuutt  iinntteerrvvaall  ==  ttookkiioo::::ttiimmee::::iinntteerrvvaall((ttookkiioo::::ttiimmee::::DDuurraattiioonn::::ffrroomm__sseeccss((6600))));;
                lleett  mmuutt  iinniittiiaalliizzeedd  ==  ffaallssee;;
                lloooopp  {{
                        iinntteerrvvaall..ttiicckk(())..aawwaaiitt;;
                        ///  FFeettcchh  bbaallaannccee  ffrroomm  PPoollyymmaarrkkeett                        mmaattcchh  bbaallaannccee__cclliieenntt..ggeett__bbaallaannccee(())..aawwaaiitt  {{
                                OOkk((bbaallaannccee))  ==>>  {{
                                        ttrraacciinngg::::iinnffoo!!((bbaallaannccee__uussddcc  ==  bbaallaannccee,,  ""üüíí∞∞  BBaallaannccee  ffeettcchheedd""));;
                                        ///  IInniittiiaalliizzee  bbaallaannccee  ttrraacckkeerr  oonn  ffiirrsstt  ffeettcchh                                        iiff  !!iinniittiiaalliizzeedd  {{
                                                bbaallaannccee__ttrraacckkeerr__cclloonnee..iinniittiiaalliizzee((bbaallaannccee));;
                                                iinniittiiaalliizzeedd  ==  ttrruuee;;
                                                ttrraacciinngg::::iinnffoo!!((iinniittiiaall__bbaallaannccee  ==  bbaallaannccee,,  ""üüèèÅÅ  BBaallaannccee  ttrraacckkeerr  iinniittiiaalliizzeedd""));;
                                        }}
                                        lleett  lloocckkeedd__iinn__ppoossiittiioonnss  ==  iiff  !!bbaallaannccee__wwaalllleett__aaddddrreessss..iiss__eemmppttyy(())  {{
                                                mmaattcchh  bbaallaannccee__cclliieenntt                                                        ..ffeettcchh__wwaalllleett__ppoossiittiioonnss((&&bbaallaannccee__wwaalllleett__aaddddrreessss))                                                        ..aawwaaiitt                                                {{
                                                        OOkk((ppoossiittiioonnss))  ==>>  ppoossiittiioonnss                                                                ..iitteerr(())                                                                ..mmaapp((||pp||  {{
                                                                        lleett  ssiizzee  ==  pp..ssiizzee..ppaarrssee::::<<ff6644>>(())..uunnwwrraapp__oorr((00..00))..mmaaxx((00..00));;
                                                                        lleett  pprriiccee  ==  pp                                                                                ..ccuurrrreenntt__pprriiccee                                                                                ..aass__rreeff(())                                                                                ..aanndd__tthheenn((||vv||  vv..ppaarrssee::::<<ff6644>>(())..ookk(())))                                                                                ..uunnwwrraapp__oorr__eellssee((||||  {{
                                                                                        pp..aavvgg__pprriiccee..ppaarrssee::::<<ff6644>>(())..uunnwwrraapp__oorr((00..00))                                                                                }}
))                                                                                ..ccllaammpp((00..00,,  11..00));;
                                                                        ssiizzee  **  pprriiccee                                                                }}
))                                                                ..ssuumm::::<<ff6644>>(()),,                                                        EErrrr((ee))  ==>>  {{
                                                                ttrraacciinngg::::wwaarrnn!!((                                                                        eerrrroorr  ==  %%ee,,                                                                        ""FFaaiilleedd  ttoo  ffeettcchh  wwaalllleett  ppoossiittiioonnss  ffoorr  lloocckkeedd--bbaallaannccee  eessttiimmaattee""                                                                ));;
                                                                00..00                                                        }}
                                                }}
                                        }}
  eellssee  {{
                                                00..00                                        }}
;;
                                        ///  UUppddaattee  bbaallaannccee  ttrraacckkeerr                                        bbaallaannccee__ttrraacckkeerr__cclloonnee..uuppddaattee__bbaallaannccee((                                                bbaallaannccee,,  ///  aavvaaiillaabbllee                                                lloocckkeedd__iinn__ppoossiittiioonnss,,                                        ));;
                                        ///  UUppddaattee  rriisskk  mmaannaaggeerr                                        bbaallaannccee__rriisskk..sseett__bbaallaannccee((bbaallaannccee));;
                                }}
                                EErrrr((ee))  ==>>  {{
                                        ttrraacciinngg::::wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  ffeettcchh  bbaallaannccee""));;
                                }}
                        }}
                }}
        }}
));;
        ///  ‚‚îîÄÄ‚‚îîÄÄ  OOrrddeerrbbooookk  ssyynncc  ttaasskk  --  ppeerriiooddiiccaallllyy  ssyynncc  ttrraacckkeerr  ttoo  ffeeaattuurree  eennggiinnee  ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ        lleett  oorrddeerrbbooookk__ssyynncc__ttrraacckkeerr  ==  oorrddeerrbbooookk__ttrraacckkeerr..cclloonnee(());;
        lleett  oorrddeerrbbooookk__ssyynncc__ffeeaattuurree  ==  ffeeaattuurree__eennggiinnee..cclloonnee(());;
        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        lleett  oorrddeerrbbooookk__ssyynncc__ddaasshhbbooaarrdd  ==  ddaasshhbbooaarrdd__mmeemmoorryy..cclloonnee(());;
        lleett  oorrddeerrbbooookk__ssyynncc__ssttrraatteeggyy  ==  ssttrraatteeggyy..cclloonnee(());;
        lleett  oorrddeerrbbooookk__ssyynncc__hhaannddllee  ==  ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                lleett  mmuutt  iinntteerrvvaall  ==  ttookkiioo::::ttiimmee::::iinntteerrvvaall((ttookkiioo::::ttiimmee::::DDuurraattiioonn::::ffrroomm__sseeccss((55))));;
                lloooopp  {{
                        iinntteerrvvaall..ttiicckk(())..aawwaaiitt;;
                        ///  UUppddaattee  ffeeaattuurree  eennggiinnee  wwiitthh  llaatteesstt  oorrddeerrbbooookk  ddaattaa                        ffoorr  aasssseett  iinn  [[AAsssseett::::BBTTCC,,  AAsssseett::::EETTHH]]  {{
                                ffoorr  ttiimmeeffrraammee  iinn  [[TTiimmeeffrraammee::::MMiinn1155,,  TTiimmeeffrraammee::::HHoouurr11]]  {{
                                        oorrddeerrbbooookk__ssyynncc__ffeeaattuurree                                                ..lloocckk(())                                                ..aawwaaiitt                                                ..uuppddaattee__ffrroomm__ttrraacckkeerr((aasssseett,,  ttiimmeeffrraammee));;
                                }}
                        }}
                        ///  UUppddaattee  ddaasshhbbooaarrdd  wwiitthh  iinnddiiccaattoorr  ssttaattss                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                        {{
                                lleett  ssttrraatt  ==  oorrddeerrbbooookk__ssyynncc__ssttrraatteeggyy..lloocckk(())..aawwaaiitt;;
                                lleett  iinnddiiccaattoorr__ssttaattss  ==  ssttrraatt..ggeett__iinnddiiccaattoorr__ssttaattss(());;
                                lleett  mmaarrkkeett__lleeaarrnniinngg  ==  ssttrraatt..eexxppoorrtt__ccaalliibbrraattoorr__ssttaattee__vv22(());;
                                lleett  ccaalliibbrraattiioonn__qquuaalliittyy  ==  ssttrraatt..eexxppoorrtt__ccaalliibbrraattiioonn__qquuaalliittyy__bbyy__mmaarrkkeett(());;
                                ddrroopp((ssttrraatt));;
                                **oorrddeerrbbooookk__ssyynncc__ddaasshhbbooaarrdd..iinnddiiccaattoorr__ssttaattss..wwrriittee(())..aawwaaiitt  ==  iinnddiiccaattoorr__ssttaattss;;
                                oorrddeerrbbooookk__ssyynncc__ddaasshhbbooaarrdd                                        ..sseett__mmaarrkkeett__lleeaarrnniinngg__ssttaattss((mmaarrkkeett__lleeaarrnniinngg))                                        ..aawwaaiitt;;
                                oorrddeerrbbooookk__ssyynncc__ddaasshhbbooaarrdd                                        ..sseett__ccaalliibbrraattiioonn__qquuaalliittyy__ssttaattss((ccaalliibbrraattiioonn__qquuaalliittyy))                                        ..aawwaaiitt;;
                        }}
                }}
        }}
));;
        ///  PPaappeerr  ttrraaddiinngg::  pprriiccee  mmoonniittoorr  ttaasskk  ((ffeeeeddss  ttiicckkss  ttoo  ppaappeerr  eennggiinnee  ffoorr  ppoossiittiioonn  ttrraacckkiinngg))        lleett  ppaappeerr__mmoonniittoorr__eennggiinnee  ==  ppaappeerr__eennggiinnee..cclloonnee(());;
        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        lleett  ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy  ==  ddaasshhbbooaarrdd__mmeemmoorryy..cclloonnee(());;
        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        lleett  ppaappeerr__ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr  ==  ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr..cclloonnee(());;
        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        lleett  ppaappeerr__ccssvv__ppeerrssiisstteennccee  ==  ccssvv__ppeerrssiisstteennccee..cclloonnee(());;
        ///  CClloonnee  ffoorr  mmaaiinn  lloooopp  ((ppaappeerr__mmoonniittoorr__hhaannddllee  ttaakkeess  oowwnneerrsshhiipp  ooff  ppaappeerr__ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr))        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]        lleett  mmaaiinn__lloooopp__bbrrooaaddccaasstteerr  ==  ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr..cclloonnee(());;
        lleett  mmaaiinn__lloooopp__sshhaarree__pprriicceess  ==  ppoollyymmaarrkkeett__sshhaarree__pprriicceess..cclloonnee(());;
        lleett  ppaappeerr__mmoonniittoorr__hhaannddllee  ==  iiff  ppaappeerr__ttrraaddiinngg__eennaabblleedd  {{
                lleett  eennggiinnee  ==  ppaappeerr__mmoonniittoorr__eennggiinnee..uunnwwrraapp(());;
                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                lleett  ccssvv__ppeerrssiisstteennccee__ffoorr__bbaacckkffiillll  ==  ppaappeerr__ccssvv__ppeerrssiisstteennccee..cclloonnee(());;
                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                lleett  mmuutt  llaasstt__pprriiccee__bbrrooaaddccaasstt::  ii6644  ==  00;;
                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                ccoonnsstt  PPRRIICCEE__BBRROOAADDCCAASSTT__IINNTTEERRVVAALL__MMSS::  ii6644  ==  11000000;;
  ///  BBrrooaaddccaasstt  pprriicceess  eevveerryy  11  sseeccoonndd  mmaaxx                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                lleett  mmuutt  llaasstt__ttrraaddee__bbaacckkffiillll__mmss::  ii6644  ==  00;;
                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                ccoonnsstt  TTRRAADDEE__BBAACCKKFFIILLLL__IINNTTEERRVVAALL__MMSS::  ii6644  ==  3300__000000;;
  ///  RReehhyyddrraattee  ttrraaddeess  ffrroomm  CCSSVV  eevveerryy  3300ss  iiff  eemmppttyy                SSoommee((ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
                        wwhhiillee  lleett  SSoommee((ttiicckk))  ==  ppaappeerr__pprriiccee__rrxx..rreeccvv(())..aawwaaiitt  {{
                                ///  UUppddaattee  tthhee  ppaappeerr  eennggiinnee  wwiitthh  tthhee  nneeww  pprriiccee                                lleett  eexxiittss  ==  eennggiinnee..uuppddaattee__pprriiccee((ttiicckk..aasssseett,,  ttiicckk..mmiidd,,  ttiicckk..ssoouurrccee));;
                                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                lleett  mmuutt  cclloosseedd__ttrraaddeess::  VVeecc<<TTrraaddeeRReessppoonnssee>>  ==  VVeecc::::nneeww(());;
                                ///  CClloossee  aannyy  ppoossiittiioonnss  tthhaatt  hhiitt  eexxiitt  ccoonnddiittiioonnss  ((wwiitthh  ffuullll  aannaallyyttiiccss))                                lleett  hhaadd__eexxiittss  ==  !!eexxiittss..iiss__eemmppttyy(());;
                                ffoorr  ((((aasssseett,,  ttiimmeeffrraammee)),,  rreeaassoonn))  iinn  eexxiittss  {{
                                        iiff  lleett  SSoommee((rreeccoorrdd))  ==  eennggiinnee..cclloossee__aanndd__ssaavvee((aasssseett,,  ttiimmeeffrraammee,,  rreeaassoonn))..aawwaaiitt  {{
                                                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                cclloosseedd__ttrraaddeess..ppuusshh((ppaappeerr__ttrraaddee__rreeccoorrdd__ttoo__ddaasshhbbooaarrdd__ttrraaddee((&&rreeccoorrdd))));;
                                        }}
                                }}
                                ///  BBrrooaaddccaasstt  ppoossiittiioonn  uuppddaatteess  iiff  aannyy  ppoossiittiioonnss  wweerree  cclloosseedd                                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                iiff  hhaadd__eexxiittss  {{
                                        ffoorr  ttrraaddee  iinn  &&cclloosseedd__ttrraaddeess  {{
                                                ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy..aadddd__ttrraaddee((ttrraaddee..cclloonnee(())))..aawwaaiitt;;
                                                ppaappeerr__ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr..bbrrooaaddccaasstt__ttrraaddee((ttrraaddee..cclloonnee(())));;
                                        }}
                                        lleett  ppoossiittiioonnss::  VVeecc<<PPoossiittiioonnRReessppoonnssee>>  ==  eennggiinnee                                                ..ggeett__ppoossiittiioonnss(())                                                ..iinnttoo__iitteerr(())                                                ..mmaapp((||pp||  PPoossiittiioonnRReessppoonnssee  {{
                                                        iidd::  pp..iidd..cclloonnee(()),,                                                        aasssseett::  ffoorrmmaatt!!((""{{
::??}}
"",,  pp..aasssseett)),,                                                        ttiimmeeffrraammee::  ffoorrmmaatt!!((""{{
::??}}
"",,  pp..ttiimmeeffrraammee)),,                                                        ddiirreeccttiioonn::  ffoorrmmaatt!!((""{{
::??}}
"",,  pp..ddiirreeccttiioonn)),,                                                        eennttrryy__pprriiccee::  pp..eennttrryy__pprriiccee,,                                                        ccuurrrreenntt__pprriiccee::  pp..ccuurrrreenntt__pprriiccee,,                                                        ssiizzee__uussddcc::  pp..ssiizzee__uussddcc,,                                                        ppnnll::  pp..uunnrreeaalliizzeedd__ppnnll,,                                                        ppnnll__ppcctt::  iiff  pp..eennttrryy__pprriiccee  >>  00..00  {{
                                                                ((((pp..ccuurrrreenntt__pprriiccee  --  pp..eennttrryy__pprriiccee))  //  pp..eennttrryy__pprriiccee  **  110000..00))                                                                        **  iiff  pp..ddiirreeccttiioonn  ====  DDiirreeccttiioonn::::UUpp  {{
                                                                                11..00                                                                        }}
  eellssee  {{
                                                                                --11..00                                                                        }}
                                                        }}
  eellssee  {{
                                                                00..00                                                        }}
,,                                                        ooppeenneedd__aatt::  pp..ooppeenneedd__aatt,,                                                        mmaarrkkeett__sslluugg::  pp..mmaarrkkeett__sslluugg..cclloonnee(()),,                                                        ccoonnffiiddeennccee::  pp..ccoonnffiiddeennccee,,                                                        ppeeaakk__pprriiccee::  pp..ppeeaakk__pprriiccee,,                                                        ttrroouugghh__pprriiccee::  pp..ttrroouugghh__pprriiccee,,                                                        mmaarrkkeett__cclloossee__ttss::  pp..mmaarrkkeett__cclloossee__ttss,,                                                        ttiimmee__rreemmaaiinniinngg__sseeccss::  ((((pp..mmaarrkkeett__cclloossee__ttss                                                                --  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp__mmiilllliiss(())))                                                                //  11000000))                                                                ..mmaaxx((00)),,                                                }}
))                                                ..ccoolllleecctt(());;
                                        **ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy..ppaappeerr__ppoossiittiioonnss..wwrriittee(())..aawwaaiitt  ==  ppoossiittiioonnss..cclloonnee(());;
                                        ppaappeerr__ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr..bbrrooaaddccaasstt__ppoossiittiioonnss((ppoossiittiioonnss..cclloonnee(())));;
                                        ///  BBrrooaaddccaasstt  ssttaattss  iimmmmeeddiiaatteellyy  aafftteerr  ppoossiittiioonn  cclloosseedd                                        lleett  ssttaattss  ==  eennggiinnee..ggeett__ssttaattss(());;
                                        lleett  bbaallaannccee  ==  eennggiinnee..ggeett__bbaallaannccee(());;
                                        lleett  lloocckkeedd  ==  eennggiinnee..ggeett__lloocckkeedd__bbaallaannccee(());;
                                        lleett  eeqquuiittyy  ==  eennggiinnee..ggeett__ttoottaall__eeqquuiittyy(());;
                                        lleett  ssttaattss__rreessppoonnssee  ==  PPaappeerrSSttaattssRReessppoonnssee  {{
                                                ttoottaall__ttrraaddeess::  ssttaattss..ttoottaall__ttrraaddeess,,                                                wwiinnss::  ssttaattss..wwiinnss,,                                                lloosssseess::  ssttaattss..lloosssseess,,                                                wwiinn__rraattee::  iiff  ssttaattss..ttoottaall__ttrraaddeess  >>  00  {{
                                                        ((ssttaattss..wwiinnss  aass  ff6644  //  ssttaattss..ttoottaall__ttrraaddeess  aass  ff6644))  **  110000..00                                                }}
  eellssee  {{
                                                        00..00                                                }}
,,                                                ttoottaall__ppnnll::  ssttaattss..ttoottaall__ppnnll,,                                                ttoottaall__ffeeeess::  ssttaattss..ttoottaall__ffeeeess,,                                                llaarrggeesstt__wwiinn::  ssttaattss..llaarrggeesstt__wwiinn,,                                                llaarrggeesstt__lloossss::  ssttaattss..llaarrggeesstt__lloossss,,                                                aavvgg__wwiinn::  iiff  ssttaattss..wwiinnss  >>  00  {{
                                                        ssttaattss..ssuumm__wwiinn__ppnnll  //  ssttaattss..wwiinnss  aass  ff6644                                                }}
  eellssee  {{
                                                        00..00                                                }}
,,                                                aavvgg__lloossss::  iiff  ssttaattss..lloosssseess  >>  00  {{
                                                        ssttaattss..ssuumm__lloossss__ppnnll  //  ssttaattss..lloosssseess  aass  ff6644                                                }}
  eellssee  {{
                                                        00..00                                                }}
,,                                                mmaaxx__ddrraawwddoowwnn::  ssttaattss..mmaaxx__ddrraawwddoowwnn,,                                                ccuurrrreenntt__ddrraawwddoowwnn::  {{
                                                        lleett  ppeeaakk  ==  ssttaattss..ppeeaakk__bbaallaannccee;;
                                                        iiff  ppeeaakk  >>  00..00  {{
                                                                ((((ppeeaakk  --  eeqquuiittyy))  //  ppeeaakk  **  110000..00))..mmaaxx((00..00))                                                        }}
  eellssee  {{
                                                                00..00                                                        }}
                                                }}
,,                                                ppeeaakk__bbaallaannccee::  ssttaattss..ppeeaakk__bbaallaannccee,,                                                pprrooffiitt__ffaaccttoorr::  iiff  ssttaattss..ggrroossss__lloossss  >>  00..00  {{
                                                        ssttaattss..ggrroossss__pprrooffiitt  //  ssttaattss..ggrroossss__lloossss                                                }}
  eellssee  iiff  ssttaattss..ggrroossss__pprrooffiitt  >>  00..00  {{
                                                        ff6644::::IINNFFIINNIITTYY                                                }}
  eellssee  {{
                                                        00..00                                                }}
,,                                                ccuurrrreenntt__ssttrreeaakk::  ssttaattss..ccuurrrreenntt__ssttrreeaakk,,                                                bbeesstt__ssttrreeaakk::  ssttaattss..bbeesstt__ssttrreeaakk,,                                                wwoorrsstt__ssttrreeaakk::  ssttaattss..wwoorrsstt__ssttrreeaakk,,                                                eexxiittss__ttrraaiilliinngg__ssttoopp::  ssttaattss..eexxiittss__ttrraaiilliinngg__ssttoopp,,                                                eexxiittss__ttaakkee__pprrooffiitt::  ssttaattss..eexxiittss__ttaakkee__pprrooffiitt,,                                                eexxiittss__mmaarrkkeett__eexxppiirryy::  ssttaattss..eexxiittss__mmaarrkkeett__eexxppiirryy,,                                                eexxiittss__ttiimmee__eexxppiirryy::  ssttaattss..eexxiittss__ttiimmee__eexxppiirryy,,                                        }}
;;
                                        ppaappeerr__ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr..bbrrooaaddccaasstt__ssttaattss((ssttaattss__rreessppoonnssee));;
                                }}
                                ///  PPeerriiooddiiccaallllyy  pprriinntt  ddaasshhbbooaarrdd                                eennggiinnee..mmaayybbee__pprriinntt__ddaasshhbbooaarrdd(());;
                                ///  ‚‚îîÄÄ‚‚îîÄÄ  UUppddaattee  DDaasshhbbooaarrdd  AAPPII  ‚‚îîÄÄ‚‚îîÄÄ                                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                {{
                                        uussee  ccrraattee::::ddaasshhbbooaarrdd::::{{
PPaappeerrSSttaattssRReessppoonnssee,,  PPoossiittiioonnRReessppoonnssee}}
;;
                                        ///  UUppddaattee  pprriiccee  iinn  ddaasshhbbooaarrdd                                        ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy                                                ..uuppddaattee__pprriiccee__aatt((                                                        ttiicckk..aasssseett,,                                                        ttiicckk..mmiidd,,                                                        ttiicckk..bbiidd,,                                                        ttiicckk..aasskk,,                                                        ttiicckk..ssoouurrccee,,                                                        ttiicckk..eexxcchhaannggee__ttss,,                                                ))                                                ..aawwaaiitt;;
                                        ///  UUppddaattee  ppaappeerr  ttrraaddiinngg  ssttaattee                                        lleett  ssttaattss  ==  eennggiinnee..ggeett__ssttaattss(());;
                                        lleett  bbaallaannccee  ==  eennggiinnee..ggeett__bbaallaannccee(());;
                                        lleett  lloocckkeedd  ==  eennggiinnee..ggeett__lloocckkeedd__bbaallaannccee(());;
                                        lleett  eeqquuiittyy  ==  eennggiinnee..ggeett__ttoottaall__eeqquuiittyy(());;
                                        lleett  uunnrreeaalliizzeedd  ==  eeqquuiittyy  --  bbaallaannccee  --  lloocckkeedd;;
                                        ///  UUppddaattee  ddaasshhbbooaarrdd  mmeemmoorryy                                        **ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy..ppaappeerr__bbaallaannccee..wwrriittee(())..aawwaaiitt  ==  bbaallaannccee;;
                                        **ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy..ppaappeerr__lloocckkeedd..wwrriittee(())..aawwaaiitt  ==  lloocckkeedd;;
                                        **ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy..ppaappeerr__uunnrreeaalliizzeedd__ppnnll..wwrriittee(())..aawwaaiitt  ==  uunnrreeaalliizzeedd;;
                                        ///  CCoonnvveerrtt  ssttaattss  ttoo  rreessppoonnssee  ttyyppee                                        lleett  ssttaattss__rreessppoonnssee  ==  PPaappeerrSSttaattssRReessppoonnssee  {{
                                                ttoottaall__ttrraaddeess::  ssttaattss..ttoottaall__ttrraaddeess,,                                                wwiinnss::  ssttaattss..wwiinnss,,                                                lloosssseess::  ssttaattss..lloosssseess,,                                                wwiinn__rraattee::  iiff  ssttaattss..ttoottaall__ttrraaddeess  >>  00  {{
                                                        ((ssttaattss..wwiinnss  aass  ff6644  //  ssttaattss..ttoottaall__ttrraaddeess  aass  ff6644))  **  110000..00                                                }}
  eellssee  {{
                                                        00..00                                                }}
,,                                                ttoottaall__ppnnll::  ssttaattss..ttoottaall__ppnnll,,                                                ttoottaall__ffeeeess::  ssttaattss..ttoottaall__ffeeeess,,                                                llaarrggeesstt__wwiinn::  ssttaattss..llaarrggeesstt__wwiinn,,                                                llaarrggeesstt__lloossss::  ssttaattss..llaarrggeesstt__lloossss,,                                                aavvgg__wwiinn::  iiff  ssttaattss..wwiinnss  >>  00  {{
                                                        ssttaattss..ssuumm__wwiinn__ppnnll  //  ssttaattss..wwiinnss  aass  ff6644                                                }}
  eellssee  {{
                                                        00..00                                                }}
,,                                                aavvgg__lloossss::  iiff  ssttaattss..lloosssseess  >>  00  {{
                                                        ssttaattss..ssuumm__lloossss__ppnnll  //  ssttaattss..lloosssseess  aass  ff6644                                                }}
  eellssee  {{
                                                        00..00                                                }}
,,                                                mmaaxx__ddrraawwddoowwnn::  ssttaattss..mmaaxx__ddrraawwddoowwnn,,                                                ccuurrrreenntt__ddrraawwddoowwnn::  {{
                                                        lleett  ppeeaakk  ==  ssttaattss..ppeeaakk__bbaallaannccee;;
                                                        iiff  ppeeaakk  >>  00..00  {{
                                                                ((((ppeeaakk  --  eeqquuiittyy))  //  ppeeaakk  **  110000..00))..mmaaxx((00..00))                                                        }}
  eellssee  {{
                                                                00..00                                                        }}
                                                }}
,,                                                ppeeaakk__bbaallaannccee::  ssttaattss..ppeeaakk__bbaallaannccee,,                                                pprrooffiitt__ffaaccttoorr::  iiff  ssttaattss..ggrroossss__lloossss  >>  00..00  {{
                                                        ssttaattss..ggrroossss__pprrooffiitt  //  ssttaattss..ggrroossss__lloossss                                                }}
  eellssee  iiff  ssttaattss..ggrroossss__pprrooffiitt  >>  00..00  {{
                                                        ff6644::::IINNFFIINNIITTYY                                                }}
  eellssee  {{
                                                        00..00                                                }}
,,                                                ccuurrrreenntt__ssttrreeaakk::  ssttaattss..ccuurrrreenntt__ssttrreeaakk,,                                                bbeesstt__ssttrreeaakk::  ssttaattss..bbeesstt__ssttrreeaakk,,                                                wwoorrsstt__ssttrreeaakk::  ssttaattss..wwoorrsstt__ssttrreeaakk,,                                                eexxiittss__ttrraaiilliinngg__ssttoopp::  ssttaattss..eexxiittss__ttrraaiilliinngg__ssttoopp,,                                                eexxiittss__ttaakkee__pprrooffiitt::  ssttaattss..eexxiittss__ttaakkee__pprrooffiitt,,                                                eexxiittss__mmaarrkkeett__eexxppiirryy::  ssttaattss..eexxiittss__mmaarrkkeett__eexxppiirryy,,                                                eexxiittss__ttiimmee__eexxppiirryy::  ssttaattss..eexxiittss__ttiimmee__eexxppiirryy,,                                        }}
;;
                                        **ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy..ppaappeerr__ssttaattss..wwrriittee(())..aawwaaiitt  ==  ssttaattss__rreessppoonnssee..cclloonnee(());;
                                        ///  UUppddaattee  ppoossiittiioonnss                                        lleett  ppoossiittiioonnss::  VVeecc<<PPoossiittiioonnRReessppoonnssee>>  ==  eennggiinnee                                                ..ggeett__ppoossiittiioonnss(())                                                ..iinnttoo__iitteerr(())                                                ..mmaapp((||pp||  PPoossiittiioonnRReessppoonnssee  {{
                                                        iidd::  pp..iidd..cclloonnee(()),,                                                        aasssseett::  ffoorrmmaatt!!((""{{
::??}}
"",,  pp..aasssseett)),,                                                        ttiimmeeffrraammee::  ffoorrmmaatt!!((""{{
::??}}
"",,  pp..ttiimmeeffrraammee)),,                                                        ddiirreeccttiioonn::  ffoorrmmaatt!!((""{{
::??}}
"",,  pp..ddiirreeccttiioonn)),,                                                        eennttrryy__pprriiccee::  pp..eennttrryy__pprriiccee,,                                                        ccuurrrreenntt__pprriiccee::  pp..ccuurrrreenntt__pprriiccee,,                                                        ssiizzee__uussddcc::  pp..ssiizzee__uussddcc,,                                                        ppnnll::  pp..uunnrreeaalliizzeedd__ppnnll,,                                                        ppnnll__ppcctt::  iiff  pp..eennttrryy__pprriiccee  >>  00..00  {{
                                                                ((((pp..ccuurrrreenntt__pprriiccee  --  pp..eennttrryy__pprriiccee))  //  pp..eennttrryy__pprriiccee  **  110000..00))                                                                        **  iiff  pp..ddiirreeccttiioonn  ====  DDiirreeccttiioonn::::UUpp  {{
                                                                                11..00                                                                        }}
  eellssee  {{
                                                                                --11..00                                                                        }}
                                                        }}
  eellssee  {{
                                                                00..00                                                        }}
,,                                                        ooppeenneedd__aatt::  pp..ooppeenneedd__aatt,,                                                        mmaarrkkeett__sslluugg::  pp..mmaarrkkeett__sslluugg..cclloonnee(()),,                                                        ccoonnffiiddeennccee::  pp..ccoonnffiiddeennccee,,                                                        ppeeaakk__pprriiccee::  pp..ppeeaakk__pprriiccee,,                                                        ttrroouugghh__pprriiccee::  pp..ttrroouugghh__pprriiccee,,                                                        mmaarrkkeett__cclloossee__ttss::  pp..mmaarrkkeett__cclloossee__ttss,,                                                        ttiimmee__rreemmaaiinniinngg__sseeccss::  ((((pp..mmaarrkkeett__cclloossee__ttss                                                                --  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp__mmiilllliiss(())))                                                                //  11000000))                                                                ..mmaaxx((00)),,                                                }}
))                                                ..ccoolllleecctt(());;
                                        **ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy..ppaappeerr__ppoossiittiioonnss..wwrriittee(())..aawwaaiitt  ==  ppoossiittiioonnss..cclloonnee(());;
                                        ///  SSaaffeettyy  nneett::  iiff  ddaasshhbbooaarrdd  ssttaarrtteedd  bbeeffoorree  CCSSVV  hhaadd  ddaattaa,,  rreehhyyddrraattee  rreecceenntt  ttrraaddeess  ffrroomm  CCSSVV..                                        lleett  nnooww  ==  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp__mmiilllliiss(());;
                                        iiff  nnooww  --  llaasstt__ttrraaddee__bbaacckkffiillll__mmss  >>==  TTRRAADDEE__BBAACCKKFFIILLLL__IINNTTEERRVVAALL__MMSS  {{
                                                llaasstt__ttrraaddee__bbaacckkffiillll__mmss  ==  nnooww;;
                                                lleett  ttrraaddeess__eemmppttyy  ==                                                        ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy..ppaappeerr__ttrraaddeess..rreeaadd(())..aawwaaiitt..iiss__eemmppttyy(());;
                                                iiff  ttrraaddeess__eemmppttyy  {{
                                                        mmaattcchh  ccssvv__ppeerrssiisstteennccee__ffoorr__bbaacckkffiillll..llooaadd__rreecceenntt__ppaappeerr__ttrraaddeess((1100__000000))  {{
                                                                OOkk((bbaacckkffiilllleedd))  iiff  !!bbaacckkffiilllleedd..iiss__eemmppttyy(())  ==>>  {{
                                                                        lleett  llooaaddeedd  ==  bbaacckkffiilllleedd..lleenn(());;
                                                                        ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy..sseett__ppaappeerr__ttrraaddeess((bbaacckkffiilllleedd))..aawwaaiitt;;
                                                                        ttrraacciinngg::::iinnffoo!!((                                                                                llooaaddeedd__ttrraaddeess  ==  llooaaddeedd,,                                                                                ""DDaasshhbbooaarrdd  rreecceenntt__ttrraaddeess  rreehhyyddrraatteedd  ffrroomm  CCSSVV""                                                                        ));;
                                                                }}
                                                                OOkk((__))  ==>>  {{
}}
                                                                EErrrr((ee))  ==>>  {{
                                                                        ttrraacciinngg::::wwaarrnn!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  bbaacckkffiillll  ddaasshhbbooaarrdd  rreecceenntt__ttrraaddeess  ffrroomm  CCSSVV""));;
                                                                }}
                                                        }}
                                                }}
                                        }}
                                        ///  BBrrooaaddccaasstt  pprriiccee  uuppddaattee  ((tthhrroottttlleedd  ttoo  pprreevveenntt  UUII  fflliicckkeerriinngg))                                        iiff  nnooww  --  llaasstt__pprriiccee__bbrrooaaddccaasstt  >>==  PPRRIICCEE__BBRROOAADDCCAASSTT__IINNTTEERRVVAALL__MMSS  {{
                                                llaasstt__pprriiccee__bbrrooaaddccaasstt  ==  nnooww;;
                                                ppaappeerr__ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr                                                        ..bbrrooaaddccaasstt__pprriicceess((ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy..ggeett__pprriicceess(())..aawwaaiitt..pprriicceess));;
                                                ppaappeerr__ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr..bbrrooaaddccaasstt__ppoossiittiioonnss((ppoossiittiioonnss..cclloonnee(())));;
                                                ///  AAllssoo  bbrrooaaddccaasstt  ssttaattss  ffoorr  rreeaall--ttiimmee  ddaasshhbbooaarrdd  uuppddaatteess                                                ppaappeerr__ddaasshhbbooaarrdd__bbrrooaaddccaasstteerr..bbrrooaaddccaasstt__ssttaattss((ssttaattss__rreessppoonnssee));;
                                        }}
                                }}
                        }}
                }}
))))        }}
  eellssee  {{
                ///  DDrraaiinn  tthhee  cchhaannnneell  ssoo  iitt  ddooeessnn''tt  bblloocckk                ttookkiioo::::ssppaawwnn((aassyynncc  mmoovvee  {{
  wwhhiillee  ppaappeerr__pprriiccee__rrxx..rreeccvv(())..aawwaaiitt..iiss__ssoommee(())  {{
}}
  }}
));;
                NNoonnee        }}
;;
        ///  MMaaiinn  lloooopp::  pprroocceessss  ssiiggnnaallss  tthhrroouugghh  rriisskk  mmaannaaggeerr  aanndd  eexxeeccuuttee        iinnffoo!!((""üüééØØ  EEnntteerriinngg  mmaaiinn  ttrraaddiinngg  lloooopp......""));;
        lloooopp  {{
                ttookkiioo::::sseelleecctt!!  {{
                        ///  PPrroocceessss  iinnccoommiinngg  ssiiggnnaallss                        SSoommee((ssiiggnnaall))  ==  ssiiggnnaall__rrxx..rreeccvv(())  ==>>  {{
                                lleett  ssiiggnnaall__iidd  ==  ssiiggnnaall..iidd..cclloonnee(());;
                                ///  ‚‚îîÄÄ‚‚îîÄÄ  PPaappeerr  TTrraaddiinngg  ppaatthh  ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ                                iiff  ppaappeerr__ttrraaddiinngg__eennaabblleedd  {{
                                        iiff  lleett  SSoommee((rreeff  eennggiinnee))  ==  ppaappeerr__eennggiinnee  {{
                                                ///  AAppppllyy  rriisskk  cchheecckkss  ((ssttiillll  uussee  rriisskk  mmaannaaggeerr  ffoorr  ssiiggnnaall  qquuaalliittyy))                                                mmaattcchh  rriisskk__mmaannaaggeerr..eevvaalluuaattee((&&ssiiggnnaall))  {{
                                                        OOkk((aapppprroovveedd))  ==>>  {{
                                                                iiff  aapppprroovveedd  {{
                                                                        iinnffoo!!((                                                                                ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                                aasssseett  ==  %%ssiiggnnaall..aasssseett,,                                                                                ddiirreeccttiioonn  ==  ??ssiiggnnaall..ddiirreeccttiioonn,,                                                                                ccoonnffiiddeennccee  ==  %%ssiiggnnaall..ccoonnffiiddeennccee,,                                                                                ""üüììãã  [[PPAAPPEERR]]  SSiiggnnaall  aapppprroovveedd""                                                                        ));;
                                                                        iiff  !!ssiiggnnaall..ttookkeenn__iidd..ttrriimm(())..iiss__eemmppttyy(())  {{
                                                                                mmaattcchh  cclloobb__cclliieenntt..qquuoottee__ttookkeenn((&&ssiiggnnaall..ttookkeenn__iidd))..aawwaaiitt  {{
                                                                                        OOkk((qq))  iiff  qq..bbiidd  >>  00..00  &&&&  qq..aasskk  >>  00..00  &&&&  qq..mmiidd  >>  00..00  ==>>  {{
                                                                                                lleett  ddiirreeccttiioonn__ssttrr  ==  mmaattcchh  ssiiggnnaall..ddiirreeccttiioonn  {{
                                                                                                        DDiirreeccttiioonn::::UUpp  ==>>  ""UUPP"",,                                                                                                        DDiirreeccttiioonn::::DDoowwnn  ==>>  ""DDOOWWNN"",,                                                                                                }}
;;
                                                                                                mmaaiinn__lloooopp__sshhaarree__pprriicceess..uuppddaattee__qquuoottee__wwiitthh__ddeepptthh((                                                                                                        ssiiggnnaall..aasssseett,,                                                                                                        ssiiggnnaall..ttiimmeeffrraammee,,                                                                                                        ddiirreeccttiioonn__ssttrr,,                                                                                                        qq..bbiidd,,                                                                                                        qq..aasskk,,                                                                                                        qq..mmiidd,,                                                                                                        qq..bbiidd__ssiizzee,,                                                                                                        qq..aasskk__ssiizzee,,                                                                                                        qq..ddeepptthh__ttoopp55,,                                                                                                ));;
                                                                                        }}
                                                                                        OOkk((__))  ==>>  {{
                                                                                                wwaarrnn!!((                                                                                                        ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                                                        ttookkeenn__iidd  ==  %%ssiiggnnaall..ttookkeenn__iidd,,                                                                                                        ""PPaappeerr  pprree--eexxeeccuuttiioonn  qquuoottee  iinnvvaalliidd;;
  kkeeeeppiinngg  eexxiissttiinngg  qquuoottee  ssttaattee""                                                                                                ));;
                                                                                        }}
                                                                                        EErrrr((ee))  ==>>  {{
                                                                                                wwaarrnn!!((                                                                                                        ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                                                        ttookkeenn__iidd  ==  %%ssiiggnnaall..ttookkeenn__iidd,,                                                                                                        eerrrroorr  ==  %%ee,,                                                                                                        ""PPaappeerr  pprree--eexxeeccuuttiioonn  qquuoottee  ffeettcchh  ffaaiilleedd;;
  kkeeeeppiinngg  eexxiissttiinngg  qquuoottee  ssttaattee""                                                                                                ));;
                                                                                        }}
                                                                                }}
                                                                        }}
                                                                        mmaattcchh  eennggiinnee..eexxeeccuuttee__ssiiggnnaall((&&ssiiggnnaall))  {{
                                                                                OOkk((ttrruuee))  ==>>  {{
                                                                                        iinnffoo!!((ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,  ""üüììãã  [[PPAAPPEERR]]  OOrrddeerr  ffiilllleedd""));;
                                                                                        ///  BBrrooaaddccaasstt  ppoossiittiioonn  ooppeenneedd  ffoorr  rreeaall--ttiimmee  ddaasshhbbooaarrdd  uuppddaatteess                                                                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                                                        {{
                                                                                                uussee  ccrraattee::::ddaasshhbbooaarrdd::::PPoossiittiioonnRReessppoonnssee;;
                                                                                                ///  GGeett  tthhee  nneewwllyy  ooppeenneedd  ppoossiittiioonn  aanndd  bbrrooaaddccaasstt  iitt                                                                                                iiff  lleett  SSoommee((ppooss))  ==  eennggiinnee..ggeett__ppoossiittiioonnss(())..iitteerr(())..ffiinndd((||pp||  pp..mmaarrkkeett__sslluugg  ====  ssiiggnnaall..mmaarrkkeett__sslluugg))  {{
                                                                                                        lleett  ppoossiittiioonn__rreessppoonnssee  ==  PPoossiittiioonnRReessppoonnssee  {{
                                                                                                                iidd::  ppooss..iidd..cclloonnee(()),,                                                                                                                aasssseett::  ffoorrmmaatt!!((""{{
::??}}
"",,  ppooss..aasssseett)),,                                                                                                                ttiimmeeffrraammee::  ffoorrmmaatt!!((""{{
::??}}
"",,  ppooss..ttiimmeeffrraammee)),,                                                                                                                ddiirreeccttiioonn::  ffoorrmmaatt!!((""{{
::??}}
"",,  ppooss..ddiirreeccttiioonn)),,                                                                                                                eennttrryy__pprriiccee::  ppooss..eennttrryy__pprriiccee,,                                                                                                                ccuurrrreenntt__pprriiccee::  ppooss..ccuurrrreenntt__pprriiccee,,                                                                                                                ssiizzee__uussddcc::  ppooss..ssiizzee__uussddcc,,                                                                                                                ppnnll::  ppooss..uunnrreeaalliizzeedd__ppnnll,,                                                                                                                ppnnll__ppcctt::  00..00,,                                                                                                                ooppeenneedd__aatt::  ppooss..ooppeenneedd__aatt,,                                                                                                                mmaarrkkeett__sslluugg::  ppooss..mmaarrkkeett__sslluugg..cclloonnee(()),,                                                                                                                ccoonnffiiddeennccee::  ppooss..ccoonnffiiddeennccee,,                                                                                                                ppeeaakk__pprriiccee::  ppooss..ppeeaakk__pprriiccee,,                                                                                                                ttrroouugghh__pprriiccee::  ppooss..ttrroouugghh__pprriiccee,,                                                                                                                mmaarrkkeett__cclloossee__ttss::  ppooss..mmaarrkkeett__cclloossee__ttss,,                                                                                                                ttiimmee__rreemmaaiinniinngg__sseeccss::  ((((ppooss..mmaarrkkeett__cclloossee__ttss  --  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp__mmiilllliiss(())))  //  11000000))..mmaaxx((00)),,                                                                                                        }}
;;
                                                                                                        mmaaiinn__lloooopp__bbrrooaaddccaasstteerr..bbrrooaaddccaasstt__ppoossiittiioonn__ooppeenneedd((ppoossiittiioonn__rreessppoonnssee));;
                                                                                                }}
                                                                                                ///  AAllssoo  bbrrooaaddccaasstt  uuppddaatteedd  ssttaattss                                                                                                lleett  ssttaattss  ==  eennggiinnee..ggeett__ssttaattss(());;
                                                                                                lleett  bbaallaannccee  ==  eennggiinnee..ggeett__bbaallaannccee(());;
                                                                                                lleett  lloocckkeedd  ==  eennggiinnee..ggeett__lloocckkeedd__bbaallaannccee(());;
                                                                                                lleett  eeqquuiittyy  ==  eennggiinnee..ggeett__ttoottaall__eeqquuiittyy(());;
                                                                                                lleett  ssttaattss__rreessppoonnssee  ==  ccrraattee::::ddaasshhbbooaarrdd::::PPaappeerrSSttaattssRReessppoonnssee  {{
                                                                                                        ttoottaall__ttrraaddeess::  ssttaattss..ttoottaall__ttrraaddeess,,                                                                                                        wwiinnss::  ssttaattss..wwiinnss,,                                                                                                        lloosssseess::  ssttaattss..lloosssseess,,                                                                                                        wwiinn__rraattee::  iiff  ssttaattss..ttoottaall__ttrraaddeess  >>  00  {{
                                                                                                                ((ssttaattss..wwiinnss  aass  ff6644  //  ssttaattss..ttoottaall__ttrraaddeess  aass  ff6644))  **  110000..00                                                                                                        }}
  eellssee  {{
  00..00  }}
,,                                                                                                        ttoottaall__ppnnll::  ssttaattss..ttoottaall__ppnnll,,                                                                                                        ttoottaall__ffeeeess::  ssttaattss..ttoottaall__ffeeeess,,                                                                                                        llaarrggeesstt__wwiinn::  ssttaattss..llaarrggeesstt__wwiinn,,                                                                                                        llaarrggeesstt__lloossss::  ssttaattss..llaarrggeesstt__lloossss,,                                                                                                        aavvgg__wwiinn::  iiff  ssttaattss..wwiinnss  >>  00  {{
  ssttaattss..ssuumm__wwiinn__ppnnll  //  ssttaattss..wwiinnss  aass  ff6644  }}
  eellssee  {{
  00..00  }}
,,                                                                                                        aavvgg__lloossss::  iiff  ssttaattss..lloosssseess  >>  00  {{
  ssttaattss..ssuumm__lloossss__ppnnll  //  ssttaattss..lloosssseess  aass  ff6644  }}
  eellssee  {{
  00..00  }}
,,                                                                                                        mmaaxx__ddrraawwddoowwnn::  ssttaattss..mmaaxx__ddrraawwddoowwnn,,                                                                                                        ccuurrrreenntt__ddrraawwddoowwnn::  {{
                                                                                                                lleett  ppeeaakk  ==  ssttaattss..ppeeaakk__bbaallaannccee;;
                                                                                                                iiff  ppeeaakk  >>  00..00  {{
  ((((ppeeaakk  --  eeqquuiittyy))  //  ppeeaakk  **  110000..00))..mmaaxx((00..00))  }}
  eellssee  {{
  00..00  }}
                                                                                                        }}
,,                                                                                                        ppeeaakk__bbaallaannccee::  ssttaattss..ppeeaakk__bbaallaannccee,,                                                                                                        pprrooffiitt__ffaaccttoorr::  iiff  ssttaattss..ggrroossss__lloossss  >>  00..00  {{
                                                                                                                ssttaattss..ggrroossss__pprrooffiitt  //  ssttaattss..ggrroossss__lloossss                                                                                                        }}
  eellssee  iiff  ssttaattss..ggrroossss__pprrooffiitt  >>  00..00  {{
  ff6644::::IINNFFIINNIITTYY  }}
  eellssee  {{
  00..00  }}
,,                                                                                                        ccuurrrreenntt__ssttrreeaakk::  ssttaattss..ccuurrrreenntt__ssttrreeaakk,,                                                                                                        bbeesstt__ssttrreeaakk::  ssttaattss..bbeesstt__ssttrreeaakk,,                                                                                                        wwoorrsstt__ssttrreeaakk::  ssttaattss..wwoorrsstt__ssttrreeaakk,,                                                                                                        eexxiittss__ttrraaiilliinngg__ssttoopp::  ssttaattss..eexxiittss__ttrraaiilliinngg__ssttoopp,,                                                                                                        eexxiittss__ttaakkee__pprrooffiitt::  ssttaattss..eexxiittss__ttaakkee__pprrooffiitt,,                                                                                                        eexxiittss__mmaarrkkeett__eexxppiirryy::  ssttaattss..eexxiittss__mmaarrkkeett__eexxppiirryy,,                                                                                                        eexxiittss__ttiimmee__eexxppiirryy::  ssttaattss..eexxiittss__ttiimmee__eexxppiirryy,,                                                                                                }}
;;
                                                                                                mmaaiinn__lloooopp__bbrrooaaddccaasstteerr..bbrrooaaddccaasstt__ssttaattss((ssttaattss__rreessppoonnssee));;
                                                                                        }}
                                                                                }}
                                                                                OOkk((ffaallssee))  ==>>  {{
                                                                                        iinnffoo!!((ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,  ""üüììãã  [[PPAAPPEERR]]  OOrrddeerr  rreejjeecctteedd  ((bbaallaannccee//ppoossiittiioonn))""));;
                                                                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                                                        ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy                                                                                                ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""ppaappeerr__eennggiinnee__rreejjeecctteedd""))                                                                                                ..aawwaaiitt;;
                                                                                }}
                                                                                EErrrr((ee))  ==>>  {{
                                                                                        eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""üüììãã  [[PPAAPPEERR]]  EExxeeccuuttee  ffaaiilleedd""));;
                                                                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                                                        ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy                                                                                                ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""ppaappeerr__eexxeeccuuttee__eerrrroorr""))                                                                                                ..aawwaaiitt;;
                                                                                }}
                                                                        }}
                                                                }}
  eellssee  {{
                                                                        iinnffoo!!((                                                                                ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                                ""‚‚èè∏∏ÔÔ∏∏èè  SSiiggnnaall  rreejjeecctteedd  bbyy  rriisskk  mmaannaaggeerr""                                                                        ));;
                                                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                                        ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy                                                                                ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""rriisskk__mmaannaaggeerr__rreejjeecctteedd""))                                                                                ..aawwaaiitt;;
                                                                }}
                                                        }}
                                                        EErrrr((ee))  ==>>  {{
                                                                eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""RRiisskk  eevvaalluuaattiioonn  ffaaiilleedd""));;
                                                                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                                ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy                                                                        ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""rriisskk__eevvaalluuaattiioonn__eerrrroorr""))                                                                        ..aawwaaiitt;;
                                                        }}
                                                }}
                                        }}
                                        ccoonnttiinnuuee;;
                                }}
                                ///  ‚‚îîÄÄ‚‚îîÄÄ  LLiivvee  //  DDrryy--RRuunn  ppaatthh  ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ‚‚îîÄÄ                                lleett  wwiinnddooww__mmss  ==  ssiiggnnaall..ttiimmeeffrraammee..dduurraattiioonn__sseeccss(())  aass  ii6644  **  11000000;;
                                lleett  wwiinnddooww__ssttaarrtt  ==  iiff  ssiiggnnaall..eexxppiirreess__aatt  >>  00  {{
                                        ssiiggnnaall..eexxppiirreess__aatt  --  wwiinnddooww__mmss                                }}
  eellssee  {{
                                        ((ssiiggnnaall..ttss  //  wwiinnddooww__mmss))  **  wwiinnddooww__mmss                                }}
;;
                                lleett  lliivvee__bbiiaass__kkeeyy  ==  ((ssiiggnnaall..aasssseett,,  ssiiggnnaall..ttiimmeeffrraammee,,  wwiinnddooww__ssttaarrtt));;
                                {{
                                        lleett  mmuutt  bbiiaass__mmaapp  ==  lliivvee__wwiinnddooww__bbiiaass..lloocckk(())..aawwaaiitt;;
                                        lleett  ccuuttooffff  ==  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp__mmiilllliiss(())                                                --  ((TTiimmeeffrraammee::::HHoouurr11..dduurraattiioonn__sseeccss(())  aass  ii6644  **  11000000  **  22));;
                                        bbiiaass__mmaapp..rreettaaiinn((||((__,,  __,,  wwss)),,  __||  **wwss  >>==  ccuuttooffff));;
                                        iiff  lleett  SSoommee((eexxiissttiinngg))  ==  bbiiaass__mmaapp..ggeett((&&lliivvee__bbiiaass__kkeeyy))  {{
                                                iiff  **eexxiissttiinngg  !!==  ssiiggnnaall..ddiirreeccttiioonn  {{
                                                        wwaarrnn!!((                                                                ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                aasssseett  ==  ??ssiiggnnaall..aasssseett,,                                                                ttiimmeeffrraammee  ==  ??ssiiggnnaall..ttiimmeeffrraammee,,                                                                wwiinnddooww__ssttaarrtt  ==  wwiinnddooww__ssttaarrtt,,                                                                eexxiissttiinngg  ==  ??eexxiissttiinngg,,                                                                iinnccoommiinngg  ==  ??ssiiggnnaall..ddiirreeccttiioonn,,                                                                ""SSkkiippppiinngg  lliivvee  ssiiggnnaall::  ooppppoossiittee  bbiiaass  aallrreeaaddyy  aaccttiivvee  ffoorr  wwiinnddooww""                                                        ));;
                                                        ccoonnttiinnuuee;;
                                                }}
                                        }}
                                }}
                                ///  AAppppllyy  rriisskk  cchheecckkss                                mmaattcchh  rriisskk__mmaannaaggeerr..eevvaalluuaattee((&&ssiiggnnaall))  {{
                                        OOkk((aapppprroovveedd))  ==>>  {{
                                                iiff  aapppprroovveedd  {{
                                                        iinnffoo!!((                                                                ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                aasssseett  ==  %%ssiiggnnaall..aasssseett,,                                                                ddiirreeccttiioonn  ==  ??ssiiggnnaall..ddiirreeccttiioonn,,                                                                ccoonnffiiddeennccee  ==  %%ssiiggnnaall..ccoonnffiiddeennccee,,                                                                ""üüììàà  SSiiggnnaall  aapppprroovveedd  bbyy  rriisskk  mmaannaaggeerr""                                                        ));;
                                                        lleett  mmaarrkkeett__sslluugg  ==  &&ssiiggnnaall..mmaarrkkeett__sslluugg;;
                                                        lleett  ttookkeenn__iidd  ==  ssiiggnnaall..ttookkeenn__iidd..cclloonnee(());;
                                                        iiff  ttookkeenn__iidd..ttrriimm(())..iiss__eemmppttyy(())  {{
                                                                wwaarrnn!!((                                                                        ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                        mmaarrkkeett__sslluugg  ==  %%mmaarrkkeett__sslluugg,,                                                                        ""CCoouulldd  nnoott  rreessoollvvee  ttookkeenn__iidd  ffrroomm  ssttrraatteeggyy  ssiiggnnaall,,  sskkiippppiinngg  oorrddeerr""                                                                ));;
                                                                ccoonnttiinnuuee;;
                                                        }}
                                                        iinnffoo!!((                                                                ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                mmaarrkkeett__sslluugg  ==  %%mmaarrkkeett__sslluugg,,                                                                ttookkeenn__iidd  ==  %%ttookkeenn__iidd,,                                                                ""üüééØØ  TTookkeenn  IIDD  rreessoollvveedd""                                                        ));;
                                                        lleett  qquuoottee  ==  mmaattcchh  cclloobb__cclliieenntt..qquuoottee__ttookkeenn((&&ttookkeenn__iidd))..aawwaaiitt  {{
                                                                OOkk((qq))  ==>>  qq,,                                                                EErrrr((ee))  ==>>  {{
                                                                        wwaarrnn!!((                                                                                ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                                ttookkeenn__iidd  ==  %%ttookkeenn__iidd,,                                                                                eerrrroorr  ==  %%ee,,                                                                                ""CCoouulldd  nnoott  qquuoottee  ttookkeenn,,  sskkiippppiinngg  oorrddeerr""                                                                        ));;
                                                                        ccoonnttiinnuuee;;
                                                                }}
                                                        }}
;;
                                                        lleett  pp__mmaarrkkeett  ==  qquuoottee..mmiidd..ccllaammpp((00..0011,,  00..9999));;
                                                        lleett  mmaaxx__sspprreeaadd  ==  mmaattcchh  ssiiggnnaall..ttiimmeeffrraammee  {{
                                                                TTiimmeeffrraammee::::MMiinn1155  ==>>  00..0033,,                                                                TTiimmeeffrraammee::::HHoouurr11  ==>>  00..0055,,                                                        }}
;;
                                                        iiff  qquuoottee..sspprreeaadd  >>  mmaaxx__sspprreeaadd  {{
                                                                iinnffoo!!((                                                                        ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                        sspprreeaadd  ==  qquuoottee..sspprreeaadd,,                                                                        mmaaxx__sspprreeaadd  ==  mmaaxx__sspprreeaadd,,                                                                        ""SSkkiippppiinngg  oorrddeerr  dduuee  ttoo  sspprreeaadd  ppoolliiccyy""                                                                ));;
                                                                ccoonnttiinnuuee;;
                                                        }}
                                                        lleett  mmiinn__ddeepptthh__ttoopp55  ==  mmaattcchh  ssiiggnnaall..ttiimmeeffrraammee  {{
                                                                TTiimmeeffrraammee::::MMiinn1155  ==>>  5500..00,,                                                                TTiimmeeffrraammee::::HHoouurr11  ==>>  2255..00,,                                                        }}
;;
                                                        iiff  qquuoottee..ddeepptthh__ttoopp55  >>  00..00  &&&&  qquuoottee..ddeepptthh__ttoopp55  <<  mmiinn__ddeepptthh__ttoopp55  {{
                                                                iinnffoo!!((                                                                        ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                        ddeepptthh__ttoopp55  ==  qquuoottee..ddeepptthh__ttoopp55,,                                                                        mmiinn__ddeepptthh__ttoopp55  ==  mmiinn__ddeepptthh__ttoopp55,,                                                                        ""SSkkiippppiinngg  oorrddeerr  dduuee  ttoo  llooww  ddeepptthh  ppoolliiccyy""                                                                ));;
                                                                ccoonnttiinnuuee;;
                                                        }}
                                                        lleett  ffeeee__rraattee  ==  ccrraattee::::ppoollyymmaarrkkeett::::ffeeee__rraattee__ffrroomm__pprriiccee((pp__mmaarrkkeett));;
                                                        lleett  eevv  ==  ccrraattee::::ppoollyymmaarrkkeett::::eessttiimmaattee__eexxppeecctteedd__vvaalluuee((                                                                pp__mmaarrkkeett,,                                                                ssiiggnnaall..ccoonnffiiddeennccee..ccllaammpp((00..0011,,  00..9999)),,                                                                pp__mmaarrkkeett,,                                                                ffeeee__rraattee,,                                                                qquuoottee..sspprreeaadd..mmaaxx((00..00)),,                                                                00..000055,,                                                        ));;
                                                        iiff  eevv..eeddggee__nneett  <<==  00..00  {{
                                                                iinnffoo!!((                                                                        ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                        eeddggee__nneett  ==  eevv..eeddggee__nneett,,                                                                        ""SSkkiippppiinngg  oorrddeerr  dduuee  ttoo  nnoonn--ppoossiittiivvee  eeddggee  aafftteerr  ccoossttss""                                                                ));;
                                                                ccoonnttiinnuuee;;
                                                        }}
                                                        lleett  nnooww__mmss  ==  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp__mmiilllliiss(());;
                                                        lleett  sseeccoonnddss__ttoo__eexxppiirryy  ==  iiff  ssiiggnnaall..eexxppiirreess__aatt  >>  00  {{
                                                                ((((ssiiggnnaall..eexxppiirreess__aatt  --  nnooww__mmss))  //  11000000))..mmaaxx((00))                                                        }}
  eellssee  {{
                                                                ii6644::::MMAAXX                                                        }}
;;
                                                        lleett  eexxeecc__ppllaann  ==  mmaattcchh  ccrraattee::::ppoollyymmaarrkkeett::::ppllaann__bbuuyy__eexxeeccuuttiioonn((                                                                qquuoottee..bbiidd,,                                                                qquuoottee..aasskk,,                                                                00..000011,,                                                                ccoonnffiigg..eexxeeccuuttiioonn..mmaakkeerr__ffiirrsstt,,                                                                ccoonnffiigg..eexxeeccuuttiioonn..ppoosstt__oonnllyy,,                                                                sseeccoonnddss__ttoo__eexxppiirryy,,                                                                ccoonnffiigg..eexxeeccuuttiioonn..ffaallllbbaacckk__ttaakkeerr__sseeccoonnddss__ttoo__eexxppiirryy,,                                                                eevv..eeddggee__nneett,,                                                        ))  {{
                                                                SSoommee((ppllaann))  ==>>  ppllaann,,                                                                NNoonnee  ==>>  {{
                                                                        wwaarrnn!!((                                                                                ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                                bbiidd  ==  qquuoottee..bbiidd,,                                                                                aasskk  ==  qquuoottee..aasskk,,                                                                                ""IInnvvaalliidd  qquuoottee  ffoorr  eexxeeccuuttiioonn  ppllaann""                                                                        ));;
                                                                        ccoonnttiinnuuee;;
                                                                }}
                                                        }}
;;
                                                        ///  AAllwwaayyss  BBUUYY  tthhee  sseelleecctteedd  oouuttccoommee  ttookkeenn..                                                        lleett  sshhaarreess__ssiizzee  ==  iiff  eexxeecc__ppllaann..eennttrryy__pprriiccee  >>  00..00  {{
                                                                ((ssiiggnnaall..ssuuggggeesstteedd__ssiizzee__uussddcc  //  eexxeecc__ppllaann..eennttrryy__pprriiccee))..mmaaxx((00..00))                                                        }}
  eellssee  {{
                                                                00..00                                                        }}
;;
                                                        iiff  sshhaarreess__ssiizzee  <<==  00..00  {{
                                                                wwaarrnn!!((                                                                        ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                        nnoottiioonnaall__uussddcc  ==  ssiiggnnaall..ssuuggggeesstteedd__ssiizzee__uussddcc,,                                                                        eennttrryy__pprriiccee  ==  eexxeecc__ppllaann..eennttrryy__pprriiccee,,                                                                        ""SSkkiippppiinngg  oorrddeerr  dduuee  ttoo  nnoonn--ppoossiittiivvee  sshhaarree  ssiizzee""                                                                ));;
                                                                ccoonnttiinnuuee;;
                                                        }}
                                                        lleett  mmuutt  oorrddeerr  ==  OOrrddeerr::::nneeww((                                                                ttookkeenn__iidd,,                                                                cclloobb::::SSiiddee::::BBuuyy,,                                                                eexxeecc__ppllaann..eennttrryy__pprriiccee,,                                                                sshhaarreess__ssiizzee,,                                                        ));;
                                                        iiff  eexxeecc__ppllaann..ppoosstt__oonnllyy  {{
                                                                oorrddeerr..eexxppiirraattiioonn  ==  ssiiggnnaall..eexxppiirreess__aatt..mmaaxx((00))  aass  uu6644;;
                                                        }}
                                                        iiff  lleett  EErrrr((ee))  ==  oorrddeerr__ttxx..sseenndd((oorrddeerr))..aawwaaiitt  {{
                                                                eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""FFaaiilleedd  ttoo  sseenndd  oorrddeerr""));;
                                                        }}
  eellssee  {{
                                                                lliivvee__wwiinnddooww__bbiiaass                                                                        ..lloocckk(())                                                                        ..aawwaaiitt                                                                        ..iinnsseerrtt((lliivvee__bbiiaass__kkeeyy,,  ssiiggnnaall..ddiirreeccttiioonn));;
                                                                ///  ‚‚îîÄÄ‚‚îîÄÄ  SSttoorree  iinnddiiccaattoorrss  uusseedd  ffoorr  tthhiiss  ppoossiittiioonn  ((ffoorr  lliivvee  ccaalliibbrraattiioonn))  ‚‚îîÄÄ‚‚îîÄÄ                                                                ///  WWhheenn  tthhee  ppoossiittiioonn  cclloosseess,,  tthhee  mmoonniittoorr  wwiillll  llooookk  tthheessee  uupp                                                                ///  ttoo  ttrraaiinn  tthhee  ccaalliibbrraattoorr                                                                iiff  !!ssiiggnnaall..iinnddiiccaattoorrss__uusseedd..iiss__eemmppttyy(())  {{
                                                                        lliivvee__iinnddiiccaattoorrss__ffoorr__mmaaiinn..lloocckk(())..aawwaaiitt..iinnsseerrtt((                                                                                ((ssiiggnnaall..aasssseett,,  ssiiggnnaall..ttiimmeeffrraammee)),,                                                                                ((ssiiggnnaall..iinnddiiccaattoorrss__uusseedd..cclloonnee(()),,  ssiiggnnaall..ccoonnffiiddeennccee..ccllaammpp((00..0011,,  00..9999)))),,                                                                        ));;
                                                                        iinnffoo!!((                                                                                ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                                aasssseett  ==  ??ssiiggnnaall..aasssseett,,                                                                                iinnddiiccaattoorrss__ccoouunntt  ==  ssiiggnnaall..iinnddiiccaattoorrss__uusseedd..lleenn(()),,                                                                                ""üüììää  [[LLIIVVEE]]  SSttoorreedd  iinnddiiccaattoorrss  ffoorr  ccaalliibbrraattiioonn  oonn  cclloossee""                                                                        ));;
                                                                }}
                                                        }}
                                                }}
  eellssee  {{
                                                        iinnffoo!!((                                                                ssiiggnnaall__iidd  ==  %%ssiiggnnaall__iidd,,                                                                ""‚‚èè∏∏ÔÔ∏∏èè  SSiiggnnaall  rreejjeecctteedd  bbyy  rriisskk  mmaannaaggeerr""                                                        ));;
                                                                        ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                                        ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy                                                                                ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""rriisskk__mmaannaaggeerr__rreejjeecctteedd""))                                                                                ..aawwaaiitt;;
                                                }}
                                        }}
                                        EErrrr((ee))  ==>>  {{
                                                eerrrroorr!!((eerrrroorr  ==  %%ee,,  ""RRiisskk  eevvaalluuaattiioonn  ffaaiilleedd""));;
                                                                ##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]                                                                ppaappeerr__ddaasshhbbooaarrdd__mmeemmoorryy                                                                        ..rreeccoorrdd__eexxeeccuuttiioonn__rreejjeeccttiioonn((""rriisskk__eevvaalluuaattiioonn__eerrrroorr""))                                                                        ..aawwaaiitt;;
                                        }}
                                }}
                        }}
                        ///  HHaannddllee  CCttrrll++CC                        __  ==  ttookkiioo::::ssiiggnnaall::::ccttrrll__cc(())  ==>>  {{
                                iinnffoo!!((""üüõõëë  SShhuuttddoowwnn  ssiiggnnaall  rreecceeiivveedd""));;
                                bbrreeaakk;;
                        }}
                }}
        }}
        ///  GGrraacceeffuull  sshhuuttddoowwnn        iinnffoo!!((""SShhuuttttiinngg  ddoowwnn......""));;
        oorraaccllee__hhaannddllee..aabboorrtt(());;
        ffeeaattuurree__hhaannddllee..aabboorrtt(());;
        ssttrraatteeggyy__hhaannddllee..aabboorrtt(());;
        eexxeeccuuttiioonn__hhaannddllee..aabboorrtt(());;
        ppoossiittiioonn__hhaannddllee..aabboorrtt(());;
        bbaallaannccee__hhaannddllee..aabboorrtt(());;
        iiff  lleett  SSoommee((hhaannddllee))  ==  ppaappeerr__mmoonniittoorr__hhaannddllee  {{
                hhaannddllee..aabboorrtt(());;
        }}
        ///  PPrriinntt  ffiinnaall  ppaappeerr  ttrraaddiinngg  ssttaattss        iiff  lleett  SSoommee((rreeff  eennggiinnee))  ==  ppaappeerr__eennggiinnee  {{
                iinnffoo!!((""üüììãã  ‚‚ïïêê‚‚ïïêê‚‚ïïêê  FFIINNAALL  PPAAPPEERR  TTRRAADDIINNGG  RREEPPOORRTT  ‚‚ïïêê‚‚ïïêê‚‚ïïêê""));;
                eennggiinnee..pprriinntt__ddaasshhbbooaarrdd(());;
                iinnffoo!!((""{{
}}
"",,  eennggiinnee..ssuummmmaarryy__ssttrriinngg(())));;
        }}
        ///  FFlluusshh  ppeennddiinngg  ddaattaa        ///  ccssvv__ppeerrssiisstteennccee  aauuttoommaattiiccaallllyy  fflluusshheess  oonn  eeaacchh  wwrriittee        iinnffoo!!((""üüëëãã  PPoollyyBBoott  ssttooppppeedd""));;
        OOkk(((())))}}
##[[ccffgg((ffeeaattuurree  ==  ""ddaasshhbbooaarrdd""))]]
ffnn  ppaappeerr__ttrraaddee__rreeccoorrdd__ttoo__ddaasshhbbooaarrdd__ttrraaddee((        rreeccoorrdd::  &&ccrraattee::::ppaappeerr__ttrraaddiinngg::::PPaappeerrTTrraaddeeRReeccoorrdd,,))  -->>  TTrraaddeeRReessppoonnssee  {{
        TTrraaddeeRReessppoonnssee  {{
                ttiimmeessttaammpp::  rreeccoorrdd..ttiimmeessttaammpp,,                ttrraaddee__iidd::  rreeccoorrdd..ttrraaddee__iidd..cclloonnee(()),,                aasssseett::  rreeccoorrdd..aasssseett..cclloonnee(()),,                ttiimmeeffrraammee::  rreeccoorrdd..ttiimmeeffrraammee..cclloonnee(()),,                ddiirreeccttiioonn::  rreeccoorrdd..ddiirreeccttiioonn..cclloonnee(()),,                ccoonnffiiddeennccee::  rreeccoorrdd..ccoonnffiiddeennccee,,                eennttrryy__pprriiccee::  rreeccoorrdd..eennttrryy__pprriiccee,,                eexxiitt__pprriiccee::  rreeccoorrdd..eexxiitt__pprriiccee,,                ssiizzee__uussddcc::  rreeccoorrdd..ssiizzee__uussddcc,,                ppnnll::  rreeccoorrdd..ppnnll,,                ppnnll__ppcctt::  rreeccoorrdd..ppnnll__ppcctt,,                rreessuulltt::  rreeccoorrdd..rreessuulltt..cclloonnee(()),,                eexxiitt__rreeaassoonn::  rreeccoorrdd..eexxiitt__rreeaassoonn..cclloonnee(()),,                hhoolldd__dduurraattiioonn__sseeccss::  rreeccoorrdd..hhoolldd__dduurraattiioonn__mmss  //  11000000,,                bbaallaannccee__aafftteerr::  rreeccoorrdd..bbaallaannccee__aafftteerr,,                rrssii__aatt__eennttrryy::  NNoonnee,,                mmaaccdd__hhiisstt__aatt__eennttrryy::  NNoonnee,,                bbbb__ppoossiittiioonn__aatt__eennttrryy::  NNoonnee,,                aaddxx__aatt__eennttrryy::  NNoonnee,,                vvoollaattiilliittyy__aatt__eennttrryy::  NNoonnee,,        }}
}}
##[[ddeerriivvee((DDeebbuugg,,  DDeeffaauulltt))]]
ssttrruucctt  RRuunnttiimmeeAArrggss  {{
        rreesseett__mmooddee::  OOppttiioonn<<SSttrriinngg>>,,        nnoo__bbaacckkuupp::  bbooooll,,}}
ffnn  ppaarrssee__rruunnttiimmee__aarrggss(())  -->>  RReessuulltt<<RRuunnttiimmeeAArrggss>>  {{
        lleett  mmuutt  aarrggss  ==  RRuunnttiimmeeAArrggss::::ddeeffaauulltt(());;
        lleett  mmuutt  iitteerr  ==  ssttdd::::eennvv::::aarrggss(())..sskkiipp((11));;
        wwhhiillee  lleett  SSoommee((aarrgg))  ==  iitteerr..nneexxtt(())  {{
                iiff  aarrgg  ====  ""----nnoo--bbaacckkuupp""  {{
                        aarrggss..nnoo__bbaacckkuupp  ==  ttrruuee;;
                        ccoonnttiinnuuee;;
                }}
                iiff  aarrgg  ====  ""----rreesseett""  {{
                        lleett  mmooddee  ==  iitteerr..nneexxtt(())..ookk__oorr__eellssee((||||  {{
                                aannyyhhooww::::aannyyhhooww!!((""----rreesseett  rreeqquuiirreess  aa  mmooddee  ((ssuuppppoorrtteedd::  hhaarrdd--aallll--hhiissttoorryy))""))                        }}
))??;;
                        aarrggss..rreesseett__mmooddee  ==  SSoommee((mmooddee));;
                        ccoonnttiinnuuee;;
                }}
                iiff  lleett  SSoommee((mmooddee))  ==  aarrgg..ssttrriipp__pprreeffiixx((""----rreesseett==""))  {{
                        iiff  mmooddee..ttrriimm(())..iiss__eemmppttyy(())  {{
                                aannyyhhooww::::bbaaiill!!((""----rreesseett  rreeqquuiirreess  aa  mmooddee  ((ssuuppppoorrtteedd::  hhaarrdd--aallll--hhiissttoorryy))""));;
                        }}
                        aarrggss..rreesseett__mmooddee  ==  SSoommee((mmooddee..ttoo__ssttrriinngg(())));;
                }}
        }}
        OOkk((aarrggss))}}
ffnn  mmaayybbee__rruunn__ssttaarrttuupp__rreesseett((ccoonnffiigg::  &&AAppppCCoonnffiigg,,  rruunnttiimmee__aarrggss::  &&RRuunnttiimmeeAArrggss))  -->>  RReessuulltt<<bbooooll>>  {{
        lleett  rreeqquueesstteedd__mmooddee  ==  rruunnttiimmee__aarrggss                ..rreesseett__mmooddee                ..aass__rreeff(())                ..mmaapp((||ss||  ss..aass__ssttrr(())))                ..oorr__eellssee((||||  {{
                        iiff  ccoonnffiigg..rreesseett..eennaabblleedd__oonn__ssttaarrtt  {{
                                SSoommee((ccoonnffiigg..rreesseett..mmooddee..aass__ssttrr(())))                        }}
  eellssee  {{
                                NNoonnee                        }}
                }}
));;
        lleett  SSoommee((mmooddee))  ==  rreeqquueesstteedd__mmooddee  eellssee  {{
                rreettuurrnn  OOkk((ffaallssee));;
        }}
;;
        lleett  nnoorrmmaalliizzeedd__mmooddee  ==  mmooddee..ttoo__aasscciiii__lloowweerrccaassee(())..rreeppllaaccee((''--'',,  ""__""));;
        iiff  nnoorrmmaalliizzeedd__mmooddee  !!==  ""hhaarrdd__aallll__hhiissttoorryy""  {{
                aannyyhhooww::::bbaaiill!!((                        ""UUnnssuuppppoorrtteedd  rreesseett  mmooddee  ''{{
}}
''..  SSuuppppoorrtteedd::  hhaarrdd__aallll__hhiissttoorryy  //  hhaarrdd--aallll--hhiissttoorryy"",,                        mmooddee                ));;
        }}
        lleett  ooppttiioonnss  ==  HHaarrddRReesseettOOppttiioonnss  {{
                nnoo__bbaacckkuupp::  rruunnttiimmee__aarrggss..nnoo__bbaacckkuupp  ||||  ccoonnffiigg..rreesseett..nnoo__bbaacckkuupp,,                ddeelleettee__pprriicceess::  ccoonnffiigg..rreesseett..ddeelleettee__pprriicceess,,                ddeelleettee__lleeaarrnniinngg__ssttaattee::  ccoonnffiigg..rreesseett..ddeelleettee__lleeaarrnniinngg__ssttaattee,,                ddeelleettee__ppaappeerr__ssttaattee::  ccoonnffiigg..rreesseett..ddeelleettee__ppaappeerr__ssttaattee,,        }}
;;
        iinnffoo!!((                mmooddee  ==  %%nnoorrmmaalliizzeedd__mmooddee,,                nnoo__bbaacckkuupp  ==  ooppttiioonnss..nnoo__bbaacckkuupp,,                ddeelleettee__pprriicceess  ==  ooppttiioonnss..ddeelleettee__pprriicceess,,                ddeelleettee__lleeaarrnniinngg__ssttaattee  ==  ooppttiioonnss..ddeelleettee__lleeaarrnniinngg__ssttaattee,,                ddeelleettee__ppaappeerr__ssttaattee  ==  ooppttiioonnss..ddeelleettee__ppaappeerr__ssttaattee,,                ddaattaa__ddiirr  ==  %%ccoonnffiigg..ppeerrssiisstteennccee..ddaattaa__ddiirr,,                ""EExxeeccuuttiinngg  ssttaarrttuupp  hhaarrdd  rreesseett""        ));;
        CCssvvPPeerrssiisstteennccee::::hhaarrdd__rreesseett__wwiitthh__ooppttiioonnss((&&ccoonnffiigg..ppeerrssiisstteennccee..ddaattaa__ddiirr,,  ooppttiioonnss))??;;
        OOkk((ttrruuee))}}
ffnn  ddeeffaauulltt__aanncchhoorr__pprriiccee((aasssseett::  AAsssseett))  -->>  ff6644  {{
        mmaattcchh  aasssseett  {{
                AAsssseett::::BBTTCC  ==>>  110000__000000..00,,                AAsssseett::::EETTHH  ==>>  33__000000..00,,                AAsssseett::::SSOOLL  ==>>  220000..00,,                AAsssseett::::XXRRPP  ==>>  11..00,,        }}
}}
ffnn  nnoorrmmaalliizzee__hhiissttoorryy__ttiimmeessttaammpp__mmss((ttss::  ii6644))  -->>  ii6644  {{
        iiff  ttss..aabbss(())  <<  110000__000000__000000__000000  {{
                ttss..ssaattuurraattiinngg__mmuull((11000000))        }}
  eellssee  {{
                ttss        }}
}}
ffnn  llooaadd__llooccaall__pprriiccee__ppooiinnttss((ddaattaa__ddiirr::  &&ssttrr,,  aasssseett::  AAsssseett,,  llooookkbbaacckk__hhoouurrss::  ii6644))  -->>  VVeecc<<((ii6644,,  ff6644))>>  {{
        uussee  cchhrroonnoo::::{{
DDuurraattiioonn,,  UUttcc}}
;;
        uussee  ssttdd::::ppaatthh::::PPaatthhBBuuff;;
        lleett  mmuutt  rroowwss::  VVeecc<<((ii6644,,  ff6644))>>  ==  VVeecc::::nneeww(());;
        lleett  nnooww  ==  UUttcc::::nnooww(());;
        lleett  ssttaarrtt__ttss  ==  nnooww..ttiimmeessttaammpp__mmiilllliiss(())  --  llooookkbbaacckk__hhoouurrss..mmaaxx((11))  **  33__660000__000000;;
        lleett  ddaayyss  ==  ((((llooookkbbaacckk__hhoouurrss..mmaaxx((11))  ++  2233))  //  2244))  ++  22;;
        lleett  pprriicceess__ddiirr  ==  PPaatthhBBuuff::::ffrroomm((ddaattaa__ddiirr))..jjooiinn((""pprriicceess""));;
        lleett  aasssseett__llaabbeell  ==  aasssseett..ttoo__ssttrriinngg(());;
        ffoorr  ddaayy__ooffffsseett  iinn  00....==ddaayyss  {{
                lleett  ddaattee  ==  nnooww  --  DDuurraattiioonn::::ddaayyss((ddaayy__ooffffsseett));;
                lleett  ffiilleennaammee  ==  ffoorrmmaatt!!((""pprriicceess__{{
}}
..ccssvv"",,  ddaattee..ffoorrmmaatt((""%%YY--%%mm--%%dd""))));;
                lleett  ppaatthh  ==  pprriicceess__ddiirr..jjooiinn((ffiilleennaammee));;
                iiff  !!ppaatthh..eexxiissttss(())  {{
                        ccoonnttiinnuuee;;
                }}
                lleett  OOkk((ccoonntteenntt))  ==  ssttdd::::ffss::::rreeaadd__ttoo__ssttrriinngg((&&ppaatthh))  eellssee  {{
                        ccoonnttiinnuuee;;
                }}
;;
                ffoorr  lliinnee  iinn  ccoonntteenntt..lliinneess(())  {{
                        lleett  ttrriimmmmeedd  ==  lliinnee..ttrriimm(());;
                        iiff  ttrriimmmmeedd..iiss__eemmppttyy(())  {{
                                ccoonnttiinnuuee;;
                        }}
                        lleett  ccoollss::  VVeecc<<&&ssttrr>>  ==  ttrriimmmmeedd..sspplliitt(('',,''))..mmaapp((ssttrr::::ttrriimm))..ccoolllleecctt(());;
                        iiff  ccoollss..lleenn(())  <<  33  {{
                                ccoonnttiinnuuee;;
                        }}
                        lleett  OOkk((rraaww__ttss))  ==  ccoollss[[00]]..ppaarrssee::::<<ii6644>>(())  eellssee  {{
                                ccoonnttiinnuuee;;
                        }}
;;
                        lleett  ttss  ==  nnoorrmmaalliizzee__hhiissttoorryy__ttiimmeessttaammpp__mmss((rraaww__ttss));;
                        iiff  ttss  <<  ssttaarrtt__ttss  {{
                                ccoonnttiinnuuee;;
                        }}
                        iiff  !!ccoollss[[11]]..eeqq__iiggnnoorree__aasscciiii__ccaassee((&&aasssseett__llaabbeell))  {{
                                ccoonnttiinnuuee;;
                        }}
                        lleett  OOkk((pprriiccee))  ==  ccoollss[[22]]..ppaarrssee::::<<ff6644>>(())  eellssee  {{
                                ccoonnttiinnuuee;;
                        }}
;;
                        iiff  !!pprriiccee..iiss__ffiinniittee(())  ||||  pprriiccee  <<==  00..00  {{
                                ccoonnttiinnuuee;;
                        }}
                        rroowwss..ppuusshh((((ttss,,  pprriiccee))));;
                }}
        }}
        rroowwss..ssoorrtt__bbyy__kkeeyy((||((ttss,,  __))||  **ttss));;
        rroowwss}}
ffnn  bbuuiilldd__ccaannddlleess__ffrroomm__ppooiinnttss((        aasssseett::  AAsssseett,,        ttiimmeeffrraammee::  TTiimmeeffrraammee,,        ppooiinnttss::  &&[[((ii6644,,  ff6644))]],,))  -->>  VVeecc<<ccrraattee::::ttyyppeess::::CCaannddllee>>  {{
        uussee  ssttdd::::ccoolllleeccttiioonnss::::BBTTrreeeeMMaapp;;
        ##[[ddeerriivvee((CClloonnee,,  CCooppyy))]]        ssttrruucctt  BBuucckkeett  {{
                ooppeenn::  ff6644,,                hhiigghh::  ff6644,,                llooww::  ff6644,,                cclloossee::  ff6644,,                ttrraaddeess::  uu6644,,        }}
        iiff  ppooiinnttss..iiss__eemmppttyy(())  {{
                rreettuurrnn  VVeecc::::nneeww(());;
        }}
        lleett  ttff__mmss  ==  ttiimmeeffrraammee..dduurraattiioonn__sseeccss(())  aass  ii6644  **  11000000;;
        lleett  mmuutt  bbuucckkeettss::  BBTTrreeeeMMaapp<<ii6644,,  BBuucckkeett>>  ==  BBTTrreeeeMMaapp::::nneeww(());;
        ffoorr  ((rraaww__ttss,,  pprriiccee))  iinn  ppooiinnttss..iitteerr(())..ccooppiieedd(())  {{
                iiff  !!pprriiccee..iiss__ffiinniittee(())  ||||  pprriiccee  <<==  00..00  {{
                        ccoonnttiinnuuee;;
                }}
                lleett  ttss  ==  nnoorrmmaalliizzee__hhiissttoorryy__ttiimmeessttaammpp__mmss((rraaww__ttss));;
                lleett  bbuucckkeett__ooppeenn  ==  ((ttss  //  ttff__mmss))  **  ttff__mmss;;
                bbuucckkeettss                        ..eennttrryy((bbuucckkeett__ooppeenn))                        ..aanndd__mmooddiiffyy((||bb||  {{
                                bb..hhiigghh  ==  bb..hhiigghh..mmaaxx((pprriiccee));;
                                bb..llooww  ==  bb..llooww..mmiinn((pprriiccee));;
                                bb..cclloossee  ==  pprriiccee;;
                                bb..ttrraaddeess  ==  bb..ttrraaddeess..ssaattuurraattiinngg__aadddd((11));;
                        }}
))                        ..oorr__iinnsseerrtt((BBuucckkeett  {{
                                ooppeenn::  pprriiccee,,                                hhiigghh::  pprriiccee,,                                llooww::  pprriiccee,,                                cclloossee::  pprriiccee,,                                ttrraaddeess::  11,,                        }}
));;
        }}
        lleett  mmuutt  ccaannddlleess  ==  VVeecc::::wwiitthh__ccaappaacciittyy((bbuucckkeettss..lleenn(())));;
        ffoorr  ((ooppeenn__ttiimmee,,  bbuucckkeett))  iinn  bbuucckkeettss  {{
                ccaannddlleess..ppuusshh((ccrraattee::::ttyyppeess::::CCaannddllee  {{
                        ooppeenn__ttiimmee,,                        cclloossee__ttiimmee::  ooppeenn__ttiimmee  ++  ttff__mmss  --  11,,                        aasssseett,,                        ttiimmeeffrraammee,,                        ooppeenn::  bbuucckkeett..ooppeenn,,                        hhiigghh::  bbuucckkeett..hhiigghh,,                        llooww::  bbuucckkeett..llooww,,                        cclloossee::  bbuucckkeett..cclloossee,,                        vvoolluummee::  00..00,,                        ttrraaddeess::  bbuucckkeett..ttrraaddeess,,                }}
));;
        }}
        ccaannddlleess}}
ffnn  ddeedduupp__ccaannddlleess__bbyy__ooppeenn__ttiimmee((ccaannddlleess::  VVeecc<<ccrraattee::::ttyyppeess::::CCaannddllee>>))  -->>  VVeecc<<ccrraattee::::ttyyppeess::::CCaannddllee>>  {{
        uussee  ssttdd::::ccoolllleeccttiioonnss::::BBTTrreeeeMMaapp;;
        lleett  mmuutt  bbyy__ooppeenn::  BBTTrreeeeMMaapp<<ii6644,,  ccrraattee::::ttyyppeess::::CCaannddllee>>  ==  BBTTrreeeeMMaapp::::nneeww(());;
        ffoorr  ccaannddllee  iinn  ccaannddlleess  {{
                bbyy__ooppeenn..iinnsseerrtt((ccaannddllee..ooppeenn__ttiimmee,,  ccaannddllee));;
        }}
        bbyy__ooppeenn..iinnttoo__vvaalluueess(())..ccoolllleecctt(())}}
aassyynncc  ffnn  bboooottssttrraapp__ppoollyymmaarrkkeett__hhiissttoorryy__ccaannddlleess((        cclliieenntt::  &&CClloobbCClliieenntt,,        mmaarrkkeett__sslluugg::  &&ssttrr,,        aasssseett::  AAsssseett,,        ttiimmeeffrraammee::  TTiimmeeffrraammee,,        aanncchhoorr__pprriiccee::  ff6644,,        iinntteerrvvaall::  ccrraattee::::cclloobb::::PPrriicceeHHiissttoorryyIInntteerrvvaall,,))  -->>  RReessuulltt<<VVeecc<<ccrraattee::::ttyyppeess::::CCaannddllee>>>>  {{
        lleett  mmuutt  mmaarrkkeett  ==  cclliieenntt..ffiinndd__mmaarrkkeett__bbyy__sslluugg((mmaarrkkeett__sslluugg))..aawwaaiitt;;
        iiff  mmaarrkkeett..iiss__nnoonnee(())  {{
                lleett  kkeeyywwoorrdd  ==  mmaattcchh  aasssseett  {{
                        AAsssseett::::BBTTCC  ==>>  ""bbiittccooiinn"",,                        AAsssseett::::EETTHH  ==>>  ""eetthheerreeuumm"",,                        AAsssseett::::SSOOLL  ==>>  ""ssoollaannaa"",,                        AAsssseett::::XXRRPP  ==>>  ""xxrrpp"",,                }}
;;
                lleett  mmuutt  ffaallllbbaacckk  ==  cclliieenntt..ffiinndd__mmaarrkkeettss((kkeeyywwoorrdd))..aawwaaiitt;;
                ffaallllbbaacckk..rreettaaiinn((||mm||  mm..aaccttiivvee));;
                ffaallllbbaacckk..ssoorrtt__bbyy((||aa,,  bb||  {{
                        lleett  tteexxtt__aa  ==  ffoorrmmaatt!!((                                ""{{
}}
  {{
}}
"",,                                aa..sslluugg..cclloonnee(())..uunnwwrraapp__oorr__ddeeffaauulltt(()),,                                aa..qquueessttiioonn..ttoo__aasscciiii__lloowweerrccaassee(())                        ));;
                        lleett  tteexxtt__bb  ==  ffoorrmmaatt!!((                                ""{{
}}
  {{
}}
"",,                                bb..sslluugg..cclloonnee(())..uunnwwrraapp__oorr__ddeeffaauulltt(()),,                                bb..qquueessttiioonn..ttoo__aasscciiii__lloowweerrccaassee(())                        ));;
                        lleett  ttff__mmaattcchh__aa  ==  ppaarrssee__ttiimmeeffrraammee__ffrroomm__mmaarrkkeett__tteexxtt((&&tteexxtt__aa))                                ..mmaapp((||ttff||  ttff  ====  ttiimmeeffrraammee))                                ..uunnwwrraapp__oorr((ffaallssee));;
                        lleett  ttff__mmaattcchh__bb  ==  ppaarrssee__ttiimmeeffrraammee__ffrroomm__mmaarrkkeett__tteexxtt((&&tteexxtt__bb))                                ..mmaapp((||ttff||  ttff  ====  ttiimmeeffrraammee))                                ..uunnwwrraapp__oorr((ffaallssee));;
                        lleett  eexxppiirryy__aa  ==  aa                                ..eenndd__ddaattee__iissoo                                ..aass__ddeerreeff(())                                ..aanndd__tthheenn((ccrraattee::::cclloobb::::CClloobbCClliieenntt::::ppaarrssee__eexxppiirryy__ttoo__ttiimmeessttaammpp))                                ..uunnwwrraapp__oorr((ii6644::::MMAAXX));;
                        lleett  eexxppiirryy__bb  ==  bb                                ..eenndd__ddaattee__iissoo                                ..aass__ddeerreeff(())                                ..aanndd__tthheenn((ccrraattee::::cclloobb::::CClloobbCClliieenntt::::ppaarrssee__eexxppiirryy__ttoo__ttiimmeessttaammpp))                                ..uunnwwrraapp__oorr((ii6644::::MMAAXX));;
                        ttff__mmaattcchh__bb                                ..ccmmpp((&&ttff__mmaattcchh__aa))                                ..tthheenn__wwiitthh((||||  eexxppiirryy__aa..ccmmpp((&&eexxppiirryy__bb))))                }}
));;
                mmaarrkkeett  ==  ffaallllbbaacckk..iinnttoo__iitteerr(())..nneexxtt(());;
        }}
        lleett  mmaarrkkeett  ==  mmaarrkkeett..ookk__oorr__eellssee((||||  aannyyhhooww::::aannyyhhooww!!((""MMaarrkkeett  sslluugg  ''{{
}}
''  nnoott  ffoouunndd"",,  mmaarrkkeett__sslluugg))))??;;
        lleett  ttookkeenn__iidd  ==  cclliieenntt                ..ffiinndd__ttookkeenn__iidd__ffoorr__ddiirreeccttiioonn((mmaarrkkeett__sslluugg,,  DDiirreeccttiioonn::::UUpp))                ..aawwaaiitt                ..oorr__eellssee((||||  {{
                        mmaarrkkeett                                ..ttookkeennss                                ..iitteerr(())                                ..ffiinndd((||tt||  {{
                                        lleett  oouutt  ==  tt..oouuttccoommee..ttoo__aasscciiii__lloowweerrccaassee(());;
                                        oouutt..ccoonnttaaiinnss((""yyeess""))  ||||  oouutt..ccoonnttaaiinnss((""uupp""))                                }}
))                                ..mmaapp((||tt||  tt..ttookkeenn__iidd..cclloonnee(())))                }}
))                ..oorr__eellssee((||||  mmaarrkkeett..ttookkeennss..ffiirrsstt(())..mmaapp((||tt||  tt..ttookkeenn__iidd..cclloonnee(())))))                ..ookk__oorr__eellssee((||||  aannyyhhooww::::aannyyhhooww!!((""NNoo  ttookkeenn  aavvaaiillaabbllee  ffoorr  mmaarrkkeett  ''{{
}}
''"",,  mmaarrkkeett__sslluugg))))??;;
        lleett  nnooww__mmss  ==  cchhrroonnoo::::UUttcc::::nnooww(())..ttiimmeessttaammpp__mmiilllliiss(());;
        lleett  llooookkbbaacckk__mmss  ==  mmaattcchh  ttiimmeeffrraammee  {{
                TTiimmeeffrraammee::::MMiinn1155  ==>>  77  **  2244  **  33__660000__000000ii6644,,                TTiimmeeffrraammee::::HHoouurr11  ==>>  3300  **  2244  **  33__660000__000000ii6644,,        }}
;;
        lleett  ppooiinnttss  ==  mmaattcchh  cclliieenntt                ..ggeett__ttookkeenn__pprriiccee__hhiissttoorryy((                        &&ttookkeenn__iidd,,                        iinntteerrvvaall,,                        SSoommee((((nnooww__mmss  --  llooookkbbaacckk__mmss))  //  11000000)),,                        SSoommee((nnooww__mmss  //  11000000)),,                        NNoonnee,,                ))                ..aawwaaiitt        {{
                OOkk((ppooiinnttss))  ==>>  ppooiinnttss,,                EErrrr((pprriimmaarryy__eerrrr))  ==>>  {{
                        ///  SSoommee  mmaarrkkeettss  rreejjeecctt  eexxpplliicciitt  ttiimmee  bboouunnddss  oonn  ssppeecciiffiicc  iinntteerrvvaallss..                        mmaattcchh  cclliieenntt                                ..ggeett__ttookkeenn__pprriiccee__hhiissttoorryy((&&ttookkeenn__iidd,,  iinntteerrvvaall,,  NNoonnee,,  NNoonnee,,  NNoonnee))                                ..aawwaaiitt                        {{
                                OOkk((ppooiinnttss))  ==>>  ppooiinnttss,,                                EErrrr((ffaallllbbaacckk__eerrrr))  ==>>  {{
                                        lleett  aalltt__mmaaxx  ==  ccrraattee::::cclloobb::::PPrriicceeHHiissttoorryyIInntteerrvvaall::::MMaaxx;;
                                        mmaattcchh  cclliieenntt                                                ..ggeett__ttookkeenn__pprriiccee__hhiissttoorryy((&&ttookkeenn__iidd,,  aalltt__mmaaxx,,  NNoonnee,,  NNoonnee,,  NNoonnee))                                                ..aawwaaiitt                                        {{
                                                OOkk((ppooiinnttss))  ==>>  ppooiinnttss,,                                                EErrrr((mmaaxx__eerrrr))  ==>>  {{
                                                        lleett  aalltt__ddaayy  ==  ccrraattee::::cclloobb::::PPrriicceeHHiissttoorryyIInntteerrvvaall::::OOnneeDDaayy;;
                                                        cclliieenntt                                                                ..ggeett__ttookkeenn__pprriiccee__hhiissttoorryy((&&ttookkeenn__iidd,,  aalltt__ddaayy,,  NNoonnee,,  NNoonnee,,  NNoonnee))                                                                ..aawwaaiitt                                                                ..mmaapp__eerrrr((||ddaayy__eerrrr||  {{
                                                                        aannyyhhooww::::aannyyhhooww!!((                                                                                ""pprriiccee  hhiissttoorryy  ffaaiilleedd  pprriimmaarryy==''{{
}}
''  ffaallllbbaacckk==''{{
}}
''  mmaaxx==''{{
}}
''  ddaayy==''{{
}}
''"",,                                                                                pprriimmaarryy__eerrrr,,                                                                                ffaallllbbaacckk__eerrrr,,                                                                                mmaaxx__eerrrr,,                                                                                ddaayy__eerrrr                                                                        ))                                                                }}
))??                                                }}
                                        }}
                                }}
                        }}
                }}
        }}
;;
        iiff  ppooiinnttss..iiss__eemmppttyy(())  {{
                rreettuurrnn  OOkk((VVeecc::::nneeww(())));;
        }}
        lleett  mmuutt  pprroobb__ppooiinnttss::  VVeecc<<((ii6644,,  ff6644))>>  ==  ppooiinnttss                ..iinnttoo__iitteerr(())                ..ffiilltteerr__mmaapp((||ppooiinntt||  {{
                        iiff  !!ppooiinntt..pp..iiss__ffiinniittee(())  ||||  ppooiinntt..pp  <<==  00..00  {{
                                rreettuurrnn  NNoonnee;;
                        }}
                        SSoommee((((nnoorrmmaalliizzee__hhiissttoorryy__ttiimmeessttaammpp__mmss((ppooiinntt..tt)),,  ppooiinntt..pp..ccllaammpp((00..00000011,,  00..99999999))))))                }}
))                ..ccoolllleecctt(());;
        pprroobb__ppooiinnttss..ssoorrtt__bbyy__kkeeyy((||((ttss,,  __))||  **ttss));;
        pprroobb__ppooiinnttss..ddeedduupp__bbyy__kkeeyy((||((ttss,,  __))||  **ttss));;
        iiff  pprroobb__ppooiinnttss..lleenn(())  <<  22  {{
                rreettuurrnn  OOkk((VVeecc::::nneeww(())));;
        }}
        ///  CCoonnvveerrtt  ttookkeenn--pprroobbaabbiilliittyy  hhiissttoorryy  iinnttoo  aa  ssppoott--lliikkee  ssyynntthheettiicc  sseerriieess  uussiinngg  rreettuurrnnss..        lleett  mmuutt  ssyynntthheettiicc::  VVeecc<<((ii6644,,  ff6644))>>  ==  VVeecc::::wwiitthh__ccaappaacciittyy((pprroobb__ppooiinnttss..lleenn(())));;
        lleett  ((ffiirrsstt__ttss,,  ffiirrsstt__pp))  ==  pprroobb__ppooiinnttss[[00]];;
        lleett  mmuutt  pprreevv__pprroobb  ==  ffiirrsstt__pp..mmaaxx((00..00000011));;
        lleett  mmuutt  ssyynntthheettiicc__pprriiccee  ==  aanncchhoorr__pprriiccee..mmaaxx((11..00));;
        ssyynntthheettiicc..ppuusshh((((ffiirrsstt__ttss,,  ssyynntthheettiicc__pprriiccee))));;
        ffoorr  ((ttss,,  pprroobb))  iinn  pprroobb__ppooiinnttss..iinnttoo__iitteerr(())..sskkiipp((11))  {{
                lleett  rraaww__rreett  ==  ((pprroobb  //  pprreevv__pprroobb))  --  11..00;;
                lleett  ssccaalleedd__rreett  ==  ((rraaww__rreett  **  00..3355))..ccllaammpp((--00..0033,,  00..0033));;
                ssyynntthheettiicc__pprriiccee  ==  ((ssyynntthheettiicc__pprriiccee  **  ((11..00  ++  ssccaalleedd__rreett))))..mmaaxx((00..0011));;
                ssyynntthheettiicc..ppuusshh((((ttss,,  ssyynntthheettiicc__pprriiccee))));;
                pprreevv__pprroobb  ==  pprroobb..mmaaxx((00..00000011));;
        }}
        OOkk((bbuuiilldd__ccaannddlleess__ffrroomm__ppooiinnttss((aasssseett,,  ttiimmeeffrraammee,,  &&ssyynntthheettiicc))))}}
ffnn  ppaarrssee__ttiimmeeffrraammee__ffrroomm__mmaarrkkeett__tteexxtt((rraaww::  &&ssttrr))  -->>  OOppttiioonn<<TTiimmeeffrraammee>>  {{
        lleett  tteexxtt  ==  rraaww..ttoo__aasscciiii__lloowweerrccaassee(());;
        iiff  tteexxtt..ccoonnttaaiinnss((""1155mm""))                ||||  tteexxtt..ccoonnttaaiinnss((""mmiinn1155""))                ||||  tteexxtt..ccoonnttaaiinnss((""mm1155""))                ||||  tteexxtt..ccoonnttaaiinnss((""1155  mmiinn""))                ||||  tteexxtt..ccoonnttaaiinnss((""1155--mmiinnuuttee""))                ||||  tteexxtt..ccoonnttaaiinnss((""1155  mmiinnuuttee""))        {{
                rreettuurrnn  SSoommee((TTiimmeeffrraammee::::MMiinn1155));;
        }}
        iiff  tteexxtt..ccoonnttaaiinnss((""11hh""))                ||||  tteexxtt..ccoonnttaaiinnss((""hhoouurr11""))                ||||  tteexxtt..ccoonnttaaiinnss((""hh11""))                ||||  tteexxtt..ccoonnttaaiinnss((""11  hhoouurr""))                ||||  tteexxtt..ccoonnttaaiinnss((""6600mm""))                ||||  tteexxtt..ccoonnttaaiinnss((""6600  mmiinn""))                ||||  tteexxtt..ccoonnttaaiinnss((""6600--mmiinnuuttee""))        {{
                rreettuurrnn  SSoommee((TTiimmeeffrraammee::::HHoouurr11));;
        }}
        NNoonnee}}
ffnn  iinniitt__llooggggiinngg(())  -->>  RReessuulltt<<(())>>  {{
        ///  DDeeffaauulltt  ttoo  IINNFFOO  lleevveell..  SSeett  RRUUSSTT__LLOOGG==ppoollyybboott==ddeebbuugg  ttoo  sseeee  vveerrbboossee  llooggss        lleett  ffiilltteerr  ==  EEnnvvFFiilltteerr::::ttrryy__ffrroomm__ddeeffaauulltt__eennvv(())..uunnwwrraapp__oorr__eellssee((||__||  EEnnvvFFiilltteerr::::nneeww((""iinnffoo""))));;
        ttrraacciinngg__ssuubbssccrriibbeerr::::rreeggiissttrryy(())                ..wwiitthh((ffiilltteerr))                ..wwiitthh((ffmmtt::::llaayyeerr(())..wwiitthh__ttaarrggeett((ffaallssee))..wwiitthh__tthhrreeaadd__iiddss((ffaallssee))))                ..wwiitthh((                        ffmmtt::::llaayyeerr(())                                ..jjssoonn(())                                ..wwiitthh__ttaarrggeett((ttrruuee))                                ..wwiitthh__wwrriitteerr((ssttdd::::iioo::::ssttddeerrrr)),,                ))                ..ttrryy__iinniitt(())??;;
        OOkk(((())))}}



