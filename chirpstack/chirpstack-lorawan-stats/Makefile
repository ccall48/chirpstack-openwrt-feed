include $(TOPDIR)/rules.mk

PKG_NAME:=chirpstack-lorawan-stats
PKG_VERSION:=0.1.0
PKG_RELEASE:=1

PKG_LICENSE:=MIT

PKG_BUILD_DEPENDS:=rust/host

include $(INCLUDE_DIR)/package.mk
include $(TOPDIR)/feeds/packages/lang/rust/rust-package.mk

# Required to build the vendored libzmq (zeromq-src).
export CXX:=$(TARGET_CXX_NOCACHE)

define Package/chirpstack-lorawan-stats
	SECTION:=chirpstack
	CATEGORY:=ChirpStack
	TITLE:=ChirpStack LoRaWAN statistics
	DEPENDS:=+libstdcpp +chirpstack-concentratord +luci-app-statistics +collectd-mod-exec
endef

define Package/chirpstack-lorawan-stats/description
	Collects LoRaWAN statistics (active devices, uplink / downlink airtime
	and packet counts) from all ChirpStack Concentratord instances and
	exposes these as graphs in LuCI Statistics.
endef

define Package/chirpstack-lorawan-stats/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_INSTALL_DIR)/bin/chirpstack-lorawan-stats $(1)/usr/bin/

	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./files/chirpstack-lorawan-stats.init $(1)/etc/init.d/chirpstack-lorawan-stats

	$(INSTALL_DIR) $(1)/etc/uci-defaults
	$(INSTALL_BIN) ./files/90-configure-chirpstack-lorawan-stats $(1)/etc/uci-defaults

	$(INSTALL_DIR) $(1)/www/luci-static/resources/statistics/rrdtool/definitions
	$(INSTALL_DATA) ./files/lorawan.js $(1)/www/luci-static/resources/statistics/rrdtool/definitions
endef

$(eval $(call RustBinPackage,chirpstack-lorawan-stats))
$(eval $(call BuildPackage,chirpstack-lorawan-stats))
